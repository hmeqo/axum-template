//! 文件上传/下载：草稿与采用两态、体积上限、流式下载头、跨用户绑定、DB 约束、缩略图派生与协商。

use std::io::Cursor;

use anyhow::Result;
use axum::http::{HeaderValue, StatusCode, header};
use image::{DynamicImage, ImageEncoder, ImageFormat, Rgb, RgbImage};

mod common;

use common::{Session, TestResponse, TestServer};

const BOUNDARY: &str = "----axum-template-test-boundary";

const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

fn multipart_typed(filename: &str, content_type: &str, payload: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{BOUNDARY}\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n\
             Content-Type: {content_type}\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(payload);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    body
}

fn multipart(filename: &str, payload: &[u8]) -> Vec<u8> {
    multipart_typed(filename, "text/plain", payload)
}

/// 400×300 的 PNG：走真实缩放路径（`inside` 到 64 档是 64×48）。
fn png_400x300() -> Vec<u8> {
    let img = RgbImage::from_fn(400, 300, |x, y| {
        Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
    });
    let mut buf = Vec::new();
    DynamicImage::ImageRgb8(img)
        .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .expect("png encodes");
    buf
}

/// 26 字节 TIFF 块：`II*\0` + IFD 偏移 8(le) + 条目数 1(le) + tag 0x0112 + type 3 + count 1 + value + next 0。
fn tiff_orientation(value: u16) -> Vec<u8> {
    let mut exif = Vec::new();
    exif.extend_from_slice(b"II*\0");
    exif.extend_from_slice(&8u32.to_le_bytes());
    exif.extend_from_slice(&1u16.to_le_bytes());
    exif.extend_from_slice(&0x0112u16.to_le_bytes());
    exif.extend_from_slice(&3u16.to_le_bytes());
    exif.extend_from_slice(&1u32.to_le_bytes());
    exif.extend_from_slice(&(value as u32).to_le_bytes());
    exif.extend_from_slice(&0u32.to_le_bytes());
    exif
}

/// 8×4 的 JPEG，EXIF Orientation = 6：显示尺寸是 4×8。
fn jpeg_8x4_rotated() -> Vec<u8> {
    let img = RgbImage::from_fn(8, 4, |x, _| {
        if x < 4 {
            Rgb([255, 0, 0])
        } else {
            Rgb([0, 0, 255])
        }
    });
    let mut buf = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 90);
    encoder
        .set_exif_metadata(tiff_orientation(6))
        .expect("jpeg accepts exif");
    encoder
        .encode_image(&DynamicImage::ImageRgb8(img))
        .expect("jpeg encodes");
    buf
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// AVIF 的 `ispe` 尺寸：`image` 的 `avif` 特性只带编码器（解码要 dav1d），因此从容器读。
fn avif_dimensions(bytes: &[u8]) -> (u32, u32) {
    assert_eq!(&bytes[4..12], b"ftypavif", "an AVIF container");
    let at = find(bytes, b"ispe").expect("ispe box");
    let fields = &bytes[at + 4..at + 16];

    (
        u32::from_be_bytes(fields[4..8].try_into().expect("width")),
        u32::from_be_bytes(fields[8..12].try_into().expect("height")),
    )
}

fn accept(value: &'static str) -> HeaderValue {
    HeaderValue::from_static(value)
}

/// `Vary` 会被多层各写一条（CORS 也写），因此按取值集合判断。
fn varies_on_accept(res: &TestResponse) -> bool {
    res.header_values("vary")
        .join(",")
        .split(',')
        .any(|value| value.trim().eq_ignore_ascii_case("accept"))
}

/// 请求体上限 = 文件上限 + 1 MiB 信封余量。
fn oversized_body(max_file_size: u64) -> Vec<u8> {
    multipart(
        "oversized.bin",
        &vec![b'a'; max_file_size as usize + 1024 * 1024 + 4096],
    )
}

async fn upload(
    server: &TestServer,
    session: &Session,
    filename: &str,
    payload: &[u8],
) -> Result<TestResponse> {
    Ok(server
        .post("/api/files")
        .cookie(session)
        .body(
            &format!("multipart/form-data; boundary={BOUNDARY}"),
            multipart(filename, payload),
        )
        .send()
        .await?
        .ok())
}

async fn upload_typed(
    server: &TestServer,
    session: &Session,
    filename: &str,
    content_type: &str,
    payload: &[u8],
) -> Result<TestResponse> {
    Ok(server
        .post("/api/files")
        .cookie(session)
        .body(
            &format!("multipart/form-data; boundary={BOUNDARY}"),
            multipart_typed(filename, content_type, payload),
        )
        .send()
        .await?
        .ok())
}

async fn attach(server: &TestServer, session: &Session, id: &str) -> Result<TestResponse> {
    Ok(server
        .post(format!("/api/files/{id}/attach"))
        .cookie(session)
        .send()
        .await?
        .ok())
}

fn id_of(response: &TestResponse) -> String {
    response.json()["id"].as_str().expect("id").to_owned()
}

#[tokio::test]
async fn a_request_body_over_the_body_limit_is_413_with_the_file_too_large_code() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server.login("files_limit", Some("admin")).await?;
    let limit = server.ctx.cfg().storage.max_file_size;

    server
        .post("/api/files")
        .cookie(&admin)
        .body(
            &format!("multipart/form-data; boundary={BOUNDARY}"),
            oversized_body(limit),
        )
        .send()
        .await?
        .expect(StatusCode::PAYLOAD_TOO_LARGE)
        .expect_code("file.too_large");

    Ok(())
}

/// 请求体仍在信封余量内、只有文件本身超限：这一趟会走到 `MediaStore` 的检查。
#[tokio::test]
async fn a_file_over_the_store_limit_is_413_with_the_file_too_large_code() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server.login("files_store_limit", Some("admin")).await?;
    let limit = server.ctx.cfg().storage.max_file_size;

    let payload = vec![b'a'; limit as usize + 4096];
    server
        .post("/api/files")
        .cookie(&admin)
        .body(
            &format!("multipart/form-data; boundary={BOUNDARY}"),
            multipart("over.bin", &payload),
        )
        .send()
        .await?
        .expect(StatusCode::PAYLOAD_TOO_LARGE)
        .expect_code("file.too_large");

    let (client, connection) =
        tokio_postgres::connect(&server.ctx.cfg().database.url, tokio_postgres::NoTls).await?;
    tokio::spawn(connection);
    let rows: i64 = client
        .query_one("SELECT count(*) FROM files", &[])
        .await?
        .get(0);
    assert_eq!(rows, 0, "a rejected upload writes no row");
    assert!(
        server.ctx.media().list().await?.is_empty(),
        "a rejected upload writes no blob"
    );

    Ok(())
}

#[tokio::test]
async fn a_draft_downloads_as_a_non_cached_attachment() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server.login("files_draft", Some("admin")).await?;
    let prefix = server.ctx.cfg().storage.media_url_prefix.clone();

    let uploaded = upload(&server, &admin, "note.txt", b"hello upload").await?;
    assert_eq!(uploaded.json()["state"], "draft");
    assert!(uploaded.json()["attached_at"].is_null());
    let id = id_of(&uploaded);

    let res = server.get(format!("{prefix}/{id}")).send().await?.ok();
    assert_eq!(res.header("cache-control"), Some("no-store"));
    assert!(res.header("etag").is_none(), "a draft carries no validator");
    assert_eq!(res.header("x-content-type-options"), Some("nosniff"));
    let disposition = res.header("content-disposition").unwrap_or_default();
    assert!(disposition.starts_with("attachment"), "{disposition}");
    assert!(
        disposition.contains("filename=\"note.txt\""),
        "{disposition}"
    );
    assert_eq!(res.bytes(), b"hello upload");

    // 草稿没有验证器：条件请求也必须给 200，不能回 304。
    let conditional = server
        .get(format!("{prefix}/{id}"))
        .header(header::IF_NONE_MATCH, HeaderValue::from_static("*"))
        .send()
        .await?
        .ok();
    assert_eq!(conditional.bytes(), b"hello upload");
    assert!(conditional.header("etag").is_none());

    Ok(())
}

#[tokio::test]
async fn attaching_makes_the_file_immutable_and_conditional() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server.login("files_attach", Some("admin")).await?;
    let prefix = server.ctx.cfg().storage.media_url_prefix.clone();

    let uploaded = upload(&server, &admin, "note.txt", b"hello upload").await?;
    let id = id_of(&uploaded);
    let hash = uploaded.json()["hash"].as_str().expect("hash").to_owned();

    let attached = attach(&server, &admin, &id).await?.json();
    assert_eq!(attached["state"], "attached");
    assert!(attached["attached_at"].is_string());

    let again = attach(&server, &admin, &id).await?.json();
    assert_eq!(
        again["attached_at"], attached["attached_at"],
        "attach is idempotent"
    );

    let res = server.get(format!("{prefix}/{id}")).send().await?.ok();
    let cache = res.header("cache-control").unwrap_or_default();
    assert!(
        cache.contains("max-age=31536000") && cache.contains("immutable"),
        "{cache}"
    );
    let etag = format!("\"{hash}\"");
    assert_eq!(res.header("etag"), Some(etag.as_str()));
    assert_eq!(res.header("x-content-type-options"), Some("nosniff"));
    assert_eq!(res.header("content-length"), Some("12"));
    assert_eq!(res.bytes(), b"hello upload");

    let not_modified = server
        .get(format!("{prefix}/{id}"))
        .header(
            header::IF_NONE_MATCH,
            HeaderValue::from_str(&etag).expect("etag is a valid header value"),
        )
        .send()
        .await?
        .expect(StatusCode::NOT_MODIFIED);
    assert!(not_modified.bytes().is_empty());
    assert_eq!(not_modified.header("etag"), Some(etag.as_str()));

    Ok(())
}

/// 非 ASCII 原名不破坏下载头：可见 ASCII 回退 + RFC 5987 双形式。
#[tokio::test]
async fn a_non_ascii_original_name_survives_the_download_header() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server.login("files_name", Some("admin")).await?;
    let prefix = server.ctx.cfg().storage.media_url_prefix.clone();
    let name = "报告 v2.txt";

    let uploaded = upload(&server, &admin, name, b"hello").await?;
    assert_eq!(uploaded.json()["original_name"], name);
    let id = id_of(&uploaded);

    let res = server.get(format!("{prefix}/{id}")).send().await?.ok();
    let disposition = res.header("content-disposition").unwrap_or_default();
    assert!(
        disposition.starts_with("attachment; filename=\""),
        "{disposition}"
    );
    assert!(disposition.contains("filename*=UTF-8''"), "{disposition}");
    assert!(disposition.is_ascii(), "{disposition}");
    assert!(
        !disposition.contains('\r') && !disposition.contains('\n'),
        "{disposition}"
    );
    assert_eq!(res.bytes(), b"hello");

    Ok(())
}

#[tokio::test]
async fn an_empty_upload_round_trips() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server.login("files_empty", Some("admin")).await?;
    let prefix = server.ctx.cfg().storage.media_url_prefix.clone();

    let uploaded = upload(&server, &admin, "empty.txt", b"").await?;
    assert_eq!(uploaded.json()["size"], 0);
    assert_eq!(uploaded.json()["hash"], EMPTY_SHA256);
    let id = id_of(&uploaded);

    let res = server.get(format!("{prefix}/{id}")).send().await?.ok();
    assert_eq!(res.header("content-length"), Some("0"));
    assert!(res.bytes().is_empty());

    attach(&server, &admin, &id).await?;
    let res = server.get(format!("{prefix}/{id}")).send().await?.ok();
    assert_eq!(
        res.header("etag"),
        Some(format!("\"{EMPTY_SHA256}\"").as_str())
    );
    assert!(res.bytes().is_empty());

    Ok(())
}

#[tokio::test]
async fn unknown_and_malformed_handles_are_rejected() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server.login("files_unknown", Some("admin")).await?;
    let prefix = server.ctx.cfg().storage.media_url_prefix.clone();
    let absent = axum_template::domain::vo::FileId::new();

    server
        .get(format!("{prefix}/{absent}"))
        .send()
        .await?
        .expect(StatusCode::NOT_FOUND);
    server
        .get(format!("{prefix}/not-a-uuid"))
        .send()
        .await?
        .expect(StatusCode::BAD_REQUEST);
    server
        .post(format!("/api/files/{absent}/attach"))
        .cookie(&admin)
        .send()
        .await?
        .expect(StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn attaching_someone_elses_file_is_403() -> Result<()> {
    let server = TestServer::start().await?;
    let owner = server.login("files_owner", Some("admin")).await?;
    let other = server.login("files_other", Some("admin")).await?;

    let uploaded = upload(&server, &owner, "note.txt", b"hello").await?;
    let id = id_of(&uploaded);

    server
        .post(format!("/api/files/{id}/attach"))
        .cookie(&other)
        .send()
        .await?
        .expect(StatusCode::FORBIDDEN);

    // 拒绝发生在任何写入之前:文件仍属于、且仍是所有者的草稿。
    let body = server
        .get("/api/files")
        .cookie(&owner)
        .send()
        .await?
        .ok()
        .json();
    let file = body["files"]
        .as_array()
        .expect("files should be an array")
        .iter()
        .find(|file| file["id"] == id.as_str())
        .expect("the owner's file must still be listed");
    assert_eq!(file["state"], "draft", "{body}");

    Ok(())
}

#[tokio::test]
async fn the_list_returns_only_the_callers_own_files_newest_first_in_both_states() -> Result<()> {
    let server = TestServer::start().await?;
    let owner = server.login("files_list_owner", Some("admin")).await?;
    let other = server.login("files_list_other", Some("admin")).await?;

    let first = id_of(&upload(&server, &owner, "first.txt", b"one").await?);
    let second = id_of(&upload(&server, &owner, "second.txt", b"two").await?);
    let third = id_of(&upload(&server, &owner, "third.txt", b"three").await?);
    attach(&server, &owner, &third).await?;
    let stranger = id_of(&upload(&server, &other, "stranger.txt", b"nope").await?);

    let body = server
        .get("/api/files")
        .cookie(&owner)
        .send()
        .await?
        .ok()
        .json();
    assert_eq!(body["total"], 3, "{body}");
    assert_eq!(body["page"], 1, "{body}");
    assert_eq!(body["per_page"], 10, "{body}");

    let files = body["files"].as_array().expect("files should be an array");
    let ids: Vec<&str> = files
        .iter()
        .map(|file| file["id"].as_str().expect("id"))
        .collect();
    assert_eq!(
        ids,
        [third.as_str(), second.as_str(), first.as_str()],
        "{body}"
    );
    assert!(ids.iter().all(|id| *id != stranger), "{body}");

    let mut states: Vec<&str> = files
        .iter()
        .map(|file| file["state"].as_str().expect("state"))
        .collect();
    states.sort_unstable();
    assert_eq!(states, ["attached", "draft", "draft"], "{body}");

    let paged = server
        .get("/api/files?page=2&per_page=2")
        .cookie(&owner)
        .send()
        .await?
        .ok()
        .json();
    assert_eq!(paged["total"], 3, "{paged}");
    assert_eq!(paged["files"][0]["id"], first, "{paged}");

    Ok(())
}

#[tokio::test]
async fn a_row_without_its_blob_fails_loudly() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server.login("files_missing_blob", Some("admin")).await?;
    let prefix = server.ctx.cfg().storage.media_url_prefix.clone();

    let uploaded = upload(&server, &admin, "note.txt", b"hello").await?;
    let id = id_of(&uploaded);

    server.ctx.media().delete(id.parse()?).await?;

    server
        .get(format!("{prefix}/{id}"))
        .send()
        .await?
        .expect(StatusCode::INTERNAL_SERVER_ERROR);

    Ok(())
}

#[tokio::test]
async fn the_database_rejects_an_inconsistent_state_row() -> Result<()> {
    let server = TestServer::start().await?;
    let url = server.ctx.cfg().database.url.clone();
    let (client, connection) = tokio_postgres::connect(&url, tokio_postgres::NoTls).await?;
    tokio::spawn(connection);

    for (state, attached_at) in [("attached", "NULL"), ("draft", "now()")] {
        let err = client
            .execute(
                &format!(
                    "INSERT INTO files (id, user_id, original_name, mime_type, file_size, hash, \
                     state, attached_at, created_at, updated_at) \
                     VALUES (gen_random_uuid(), 1, 'a.txt', 'text/plain', 1, 'h', \
                     '{state}', {attached_at}, now(), now())"
                ),
                &[],
            )
            .await
            .expect_err("the check constraint must reject inconsistent state columns");

        let db_error = err.as_db_error().expect("a database error");
        assert_eq!(
            db_error.code(),
            &tokio_postgres::error::SqlState::CHECK_VIOLATION,
            "{state} / {attached_at}: {db_error}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn file_endpoints_need_user_write() -> Result<()> {
    let server = TestServer::start().await?;
    let anonymous = server.login("files_anon", None).await?;

    let listed = server
        .get("/api/files")
        .cookie(&anonymous)
        .send()
        .await?
        .expect(StatusCode::FORBIDDEN);
    assert!(listed.json()["code"].is_null(), "{}", listed.text());

    let uploaded = server
        .post("/api/files")
        .cookie(&anonymous)
        .body(
            &format!("multipart/form-data; boundary={BOUNDARY}"),
            multipart("a.txt", b"hi"),
        )
        .send()
        .await?
        .expect(StatusCode::FORBIDDEN);
    assert!(uploaded.json()["code"].is_null(), "{}", uploaded.text());

    let attached = server
        .post("/api/files/00000000-0000-0000-0000-000000000000/attach")
        .cookie(&anonymous)
        .send()
        .await?
        .expect(StatusCode::FORBIDDEN);
    assert!(attached.json()["code"].is_null(), "{}", attached.text());

    Ok(())
}

#[tokio::test]
async fn a_thumbnail_is_generated_then_served_from_disk() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server.login("thumbs_generate", Some("admin")).await?;
    let prefix = server.ctx.cfg().storage.media_url_prefix.clone();

    let uploaded = upload_typed(&server, &admin, "photo.png", "image/png", &png_400x300()).await?;
    let id = id_of(&uploaded);
    let uri = format!("{prefix}/{id}?size=64");

    let first = server
        .get(&uri)
        .header(header::ACCEPT, accept("image/avif"))
        .send()
        .await?
        .ok();
    assert_eq!(first.header("content-type"), Some("image/avif"));
    assert_eq!(
        first.header("content-disposition"),
        Some("inline"),
        "a derivative renders"
    );
    assert!(varies_on_accept(&first), "the derivative varies on Accept");
    assert_eq!(first.header("x-content-type-options"), Some("nosniff"));
    assert_eq!(
        first.header("cache-control"),
        Some("no-store"),
        "a draft is never cached"
    );
    assert!(
        first.header("etag").is_none(),
        "a draft carries no validator"
    );
    assert_eq!(avif_dimensions(first.bytes()), (64, 48), "inside, no crop");
    let len: usize = first
        .header("content-length")
        .expect("content-length")
        .parse()?;
    assert_eq!(len, first.bytes().len(), "the length is the derivative's");

    let derivatives = server.ctx.media().list_derivatives(id.parse()?).await?;
    assert!(
        derivatives.contains(&"v1-64.avif".to_string()),
        "{derivatives:?}"
    );

    // 命中磁盘：同一请求拿到同一份字节。
    let second = server
        .get(&uri)
        .header(header::ACCEPT, accept("image/avif"))
        .send()
        .await?
        .ok();
    assert_eq!(second.bytes(), first.bytes());

    // 采用之后 immutable + 派生验证器（带管线版本、档位与格式）。
    attach(&server, &admin, &id).await?;
    let hash = uploaded.json()["hash"].as_str().expect("hash").to_owned();
    let etag = format!("\"{hash}-v1-64-avif\"");

    let cached = server
        .get(&uri)
        .header(header::ACCEPT, accept("image/avif"))
        .send()
        .await?
        .ok();
    let cache = cached.header("cache-control").unwrap_or_default();
    assert!(
        cache.contains("max-age=31536000") && cache.contains("immutable"),
        "{cache}"
    );
    assert_eq!(cached.header("etag"), Some(etag.as_str()));
    assert_eq!(cached.header("content-disposition"), Some("inline"));

    let not_modified = server
        .get(&uri)
        .header(header::ACCEPT, accept("image/avif"))
        .header(
            header::IF_NONE_MATCH,
            HeaderValue::from_str(&etag).expect("etag is a valid header value"),
        )
        .send()
        .await?
        .expect(StatusCode::NOT_MODIFIED);
    assert!(not_modified.bytes().is_empty());
    assert_eq!(not_modified.header("etag"), Some(etag.as_str()));
    assert!(
        varies_on_accept(&not_modified),
        "a 304 keeps the cache-relevant headers"
    );

    Ok(())
}

#[tokio::test]
async fn switching_accept_switches_the_derivative() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server.login("thumbs_accept", Some("admin")).await?;
    let prefix = server.ctx.cfg().storage.media_url_prefix.clone();

    let uploaded = upload_typed(&server, &admin, "photo.png", "image/png", &png_400x300()).await?;
    let id = id_of(&uploaded);
    attach(&server, &admin, &id).await?;
    let uri = format!("{prefix}/{id}?size=64");

    let avif = server
        .get(&uri)
        .header(header::ACCEPT, accept("image/avif"))
        .send()
        .await?
        .ok();
    let jpeg = server
        .get(&uri)
        .header(header::ACCEPT, accept("image/jpeg"))
        .send()
        .await?
        .ok();

    assert_eq!(jpeg.header("content-type"), Some("image/jpeg"));
    assert_ne!(
        jpeg.header("etag"),
        avif.header("etag"),
        "the validator names the format"
    );
    let decoded = image::load_from_memory(jpeg.bytes()).expect("the jpeg derivative decodes");
    assert_eq!((decoded.width(), decoded.height()), (64, 48));

    let derivatives = server.ctx.media().list_derivatives(id.parse()?).await?;
    for name in ["v1-64.avif", "v1-64.jpg"] {
        assert!(derivatives.contains(&name.to_string()), "{derivatives:?}");
    }

    // 没有可接受的档位（只声明 png）时按 JPEG 兜底，而不是 406。
    let fallback = server
        .get(&uri)
        .header(header::ACCEPT, accept("image/png"))
        .send()
        .await?
        .ok();
    assert_eq!(fallback.header("content-type"), Some("image/jpeg"));

    Ok(())
}

#[tokio::test]
async fn a_small_source_is_never_enlarged() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server.login("thumbs_small", Some("admin")).await?;
    let prefix = server.ctx.cfg().storage.media_url_prefix.clone();

    let uploaded = upload_typed(
        &server,
        &admin,
        "rotated.jpg",
        "image/jpeg",
        &jpeg_8x4_rotated(),
    )
    .await?;
    let id = id_of(&uploaded);

    // 派生图按 EXIF 方向给出显示尺寸，因此 8×4 的源是 4×8。
    assert_eq!(uploaded.json()["width"], 4);
    assert_eq!(uploaded.json()["height"], 8);

    let res = server
        .get(format!("{prefix}/{id}?size=512"))
        .header(header::ACCEPT, accept("image/avif"))
        .send()
        .await?
        .ok();
    assert_eq!(
        avif_dimensions(res.bytes()),
        (4, 8),
        "no upscale past the source"
    );
    assert!(
        res.bytes().len() < 4096,
        "still re-encoded, not the original"
    );

    let derivatives = server.ctx.media().list_derivatives(id.parse()?).await?;
    assert!(
        derivatives.contains(&"v1-512.avif".to_string()),
        "{derivatives:?}"
    );

    Ok(())
}

#[tokio::test]
async fn thumbnail_gates_answer_400_without_a_code() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server.login("thumbs_gates", Some("admin")).await?;
    let prefix = server.ctx.cfg().storage.media_url_prefix.clone();

    let text = upload(&server, &admin, "note.txt", b"hello").await?;
    let id = id_of(&text);

    for query in ["size=64", "size=63", "size=0", "size=abc", "size="] {
        let res = server
            .get(format!("{prefix}/{id}?{query}"))
            .send()
            .await?
            .expect(StatusCode::BAD_REQUEST);
        assert!(res.json()["code"].is_null(), "{query}: {}", res.text());
    }

    // 非法 `size` 先于行查找：不泄露句柄是否存在。
    server
        .get(format!("{prefix}/not-a-uuid?size=64"))
        .send()
        .await?
        .expect(StatusCode::BAD_REQUEST);

    // 不存在的手柄 + 合法档位 = 404。
    let absent = axum_template::domain::vo::FileId::new();
    server
        .get(format!("{prefix}/{absent}?size=64"))
        .send()
        .await?
        .expect(StatusCode::NOT_FOUND);

    Ok(())
}

/// 不带 `size` 的请求走原件路径：栅格原件 `inline`、行里的类型与长度、sha256 验证器，且不随 `Accept` 变化。
#[tokio::test]
async fn a_raster_original_is_inlined_and_does_not_vary_on_accept() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server.login("thumbs_original", Some("admin")).await?;
    let prefix = server.ctx.cfg().storage.media_url_prefix.clone();

    let uploaded = upload_typed(&server, &admin, "photo.png", "image/png", &png_400x300()).await?;
    let id = id_of(&uploaded);
    attach(&server, &admin, &id).await?;
    let hash = uploaded.json()["hash"].as_str().expect("hash").to_owned();

    let res = server.get(format!("{prefix}/{id}")).send().await?.ok();
    let disposition = res.header("content-disposition").unwrap_or_default();

    assert!(
        disposition.starts_with("inline; filename=\"photo.png\""),
        "a raster original renders in place (nosniff keeps the declared type): {disposition}"
    );
    assert_eq!(res.header("content-type"), Some("image/png"));
    assert_eq!(res.header("etag"), Some(format!("\"{hash}\"").as_str()));
    assert!(
        !varies_on_accept(&res),
        "the original does not vary on Accept: {:?}",
        res.header_values("vary")
    );
    let len: usize = res
        .header("content-length")
        .expect("content-length")
        .parse()?;
    assert_eq!(len, uploaded.json()["size"].as_u64().unwrap() as usize);
    assert_eq!(res.bytes().len(), len);

    Ok(())
}

#[tokio::test]
async fn the_list_publishes_thumbnail_urls_and_dimensions() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server.login("thumbs_list", Some("admin")).await?;
    let prefix = server.ctx.cfg().storage.media_url_prefix.clone();

    let image =
        id_of(&upload_typed(&server, &admin, "photo.png", "image/png", &png_400x300()).await?);
    let text = id_of(&upload(&server, &admin, "note.txt", b"hello").await?);

    let body = server
        .get("/api/files")
        .cookie(&admin)
        .send()
        .await?
        .ok()
        .json();
    let files = body["files"].as_array().expect("files should be an array");
    let find = |id: &str| {
        files
            .iter()
            .find(|file| file["id"] == id)
            .unwrap_or_else(|| panic!("{id} must be listed: {body}"))
            .clone()
    };

    let image = find(&image);
    assert_eq!(
        image["thumbnail_url"],
        format!(
            "{prefix}/{image_id}?size=256",
            image_id = image["id"].as_str().unwrap()
        )
    );
    assert_eq!(image["width"], 400);
    assert_eq!(image["height"], 300);

    let text = find(&text);
    assert!(text["thumbnail_url"].is_null(), "{text}");
    assert!(text["width"].is_null(), "{text}");
    assert!(text["height"].is_null(), "{text}");

    Ok(())
}
