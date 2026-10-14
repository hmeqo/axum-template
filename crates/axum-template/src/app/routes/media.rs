//! `{storage.media-url-prefix}`：公开的原图与派生图下发；`?size=` 走服务端 `Accept` 协商。

use axum::{
    Router,
    extract::State,
    http::{HeaderMap, header},
    response::Response,
    routing::get,
};

use crate::{
    app::{
        AppContext,
        dto::FileIdPath,
        extractor::{AppPath, AppQuery},
        http::response::Serving,
    },
    bail,
    domain::vo::{ThumbFormat, ThumbSize},
    error::AppError,
};

/// `size` 不在白名单、非数字、为空都是 400（`AppQuery` 的 serde 失败即无 code 的 400）。
#[derive(Debug, serde::Deserialize, validator::Validate)]
pub struct ThumbQuery {
    size: Option<u32>,
}

pub async fn download(
    State(ctx): State<AppContext>,
    AppPath(FileIdPath { id }): AppPath<FileIdPath>,
    AppQuery(query): AppQuery<ThumbQuery>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    // 不带 `size`：原件路径（栅格图 `inline`，其余 `attachment`——同源内联渲染用户上传物即存储型 XSS）。
    let Some(requested) = query.size else {
        let Some(stored) = ctx.srv().file.find_by_id(id).await? else {
            bail!(AppError::not_found("File not found"));
        };

        let serving = Serving::of(&stored);
        if let Some(response) = serving.not_modified(&headers) {
            return Ok(response);
        }

        let Some(blob) = ctx.media().open(id).await? else {
            bail!(AppError::internal(format!("file blob is missing: {id}")));
        };

        return Ok(serving.stream(blob));
    };

    // 派生图路径：`FileService::thumbnail` 自己处理「行不存在 → 404 / 非栅格 → 400」，这里不预取行。
    let size = ThumbSize::parse(requested).ok_or_else(|| {
        AppError::malformed_request(format!("Unsupported thumbnail size: {requested}"))
    })?;

    let thumb = ctx
        .srv()
        .file
        .thumbnail(id, size, negotiate(&headers))
        .await?;
    let serving = Serving::thumbnail(&thumb.file, thumb.size, thumb.format, thumb.len);
    if let Some(response) = serving.not_modified(&headers) {
        return Ok(response);
    }

    Ok(serving.stream(thumb.body))
}

/// 协商结果：AVIF 更小，JPEG 兜底；同 q 时按这个顺序。`Accept` 缺失或 `*/*` 视为接受任一档；
/// 没有任何可接受的档位（例如只声明 `image/png`）时按 JPEG 兜底，不返回 406。
fn negotiate(headers: &HeaderMap) -> ThumbFormat {
    const PREFERENCE: [ThumbFormat; 2] = [ThumbFormat::Avif, ThumbFormat::Jpeg];

    let Some(accept) = headers
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
    else {
        return ThumbFormat::Avif;
    };

    let mut best: Option<(f32, usize)> = None;
    for entry in accept.split(',') {
        let mut fields = entry.split(';');
        let media = fields.next().unwrap_or_default().trim();
        let q = fields
            .filter_map(|field| field.trim().strip_prefix("q="))
            .next()
            .and_then(|q| q.trim().parse::<f32>().ok())
            .unwrap_or(1.0);
        if q <= 0.0 {
            continue;
        }

        let wildcard = media == "*/*" || media == "image/*";
        for (rank, format) in PREFERENCE.iter().enumerate() {
            let named = match format {
                ThumbFormat::Avif => media == "image/avif",
                ThumbFormat::Jpeg => media == "image/jpeg" || media == "image/jpg",
            };
            if !wildcard && !named {
                continue;
            }

            if best
                .is_none_or(|(best_q, best_rank)| q > best_q || (q == best_q && rank < best_rank))
            {
                best = Some((q, rank));
            }
        }
    }

    match best {
        Some((_, 0)) => ThumbFormat::Avif,
        Some(_) => ThumbFormat::Jpeg,
        None => ThumbFormat::Jpeg,
    }
}

/// 挂载点由配置决定（`storage.media-url-prefix`），因此本模块自带前缀，`create_router` 只 merge。
pub fn router(ctx: &AppContext) -> Router<AppContext> {
    Router::new().route(
        &format!("{}/{{id}}", ctx.cfg().storage.media_url_prefix),
        get(download),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(accept: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT, accept.parse().expect("a valid header"));
        headers
    }

    #[test]
    fn negotiation_follows_the_client_preference_then_the_server_rank() {
        // 浏览器的典型头：两者都收，服务端偏好 AVIF。
        assert_eq!(
            negotiate(&headers(
                "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8"
            )),
            ThumbFormat::Avif
        );
        assert_eq!(
            negotiate(&headers("image/jpeg,image/png")),
            ThumbFormat::Jpeg
        );
        assert_eq!(negotiate(&headers("*/*")), ThumbFormat::Avif);
        assert_eq!(negotiate(&headers("image/*")), ThumbFormat::Avif);

        // q 值说了算：明确要 JPEG 且把通配压低。
        assert_eq!(
            negotiate(&headers("image/jpeg,image/*;q=0.5")),
            ThumbFormat::Jpeg
        );
        assert_eq!(
            negotiate(&headers("image/jpeg;q=0.1,image/avif;q=0.9")),
            ThumbFormat::Avif
        );
        // q=0 即不收。
        assert_eq!(
            negotiate(&headers("image/avif;q=0,image/jpeg")),
            ThumbFormat::Jpeg
        );
    }

    #[test]
    fn negotiation_falls_back_to_jpeg_instead_of_failing() {
        assert_eq!(negotiate(&HeaderMap::new()), ThumbFormat::Avif);
        assert_eq!(negotiate(&headers("")), ThumbFormat::Jpeg);
        assert_eq!(negotiate(&headers("image/png")), ThumbFormat::Jpeg);
        assert_eq!(negotiate(&headers("text/html,*/*;q=0")), ThumbFormat::Jpeg);
    }
}
