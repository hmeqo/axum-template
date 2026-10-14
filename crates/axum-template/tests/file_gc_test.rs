//! 文件 GC：未采用草稿回收与盘行对账。

use std::time::Duration;

use anyhow::{Context, Result};

mod common;

use axum_template::domain::{
    service::{ReconcileReport, StoredFile},
    vo::{FileId, UserId},
};
use common::TestServer;

/// 本文件里“过期”的量级：TTL/grace 取 TTL，被拨老的取 AGE。
const TTL: Duration = Duration::from_secs(3600);
const AGE: Duration = Duration::from_secs(2 * 3600);

async fn backdate_row(server: &TestServer, id: FileId, age: Duration) -> Result<()> {
    // ORM 不让伪造 `created_at`，故直连库改；过期由数据决定而不是靠 sleep，TTL 的比较才真的被验证。
    let url = &server.ctx.cfg().database.url;
    let (client, connection) = tokio_postgres::connect(url, tokio_postgres::NoTls)
        .await
        .context("backdating a row needs a direct connection")?;
    tokio::spawn(connection);

    let updated = client
        .execute(
            "UPDATE files SET created_at = now() - ($2::text)::interval WHERE id = $1::text::uuid",
            &[&id.to_string(), &format!("{} seconds", age.as_secs())],
        )
        .await?;
    assert_eq!(updated, 1, "expected exactly one row to backdate");

    Ok(())
}

/// 把盘上原文件的 mtime 拨到 `age` 之前（对账的宽限期看的就是 `origin` 的 mtime）。
fn backdate_blob(server: &TestServer, id: FileId, age: Duration) -> Result<()> {
    let path = server.ctx.media().origin_path(id);
    std::fs::File::open(&path)
        .with_context(|| format!("cannot open the blob at {}", path.display()))?
        .set_modified(std::time::SystemTime::now() - age)?;

    Ok(())
}

async fn upload(server: &TestServer, owner: UserId, name: &str, data: &[u8]) -> Result<StoredFile> {
    Ok(server
        .ctx
        .srv()
        .file
        .upload(owner, name.to_owned(), "text/plain".to_owned(), data)
        .await?)
}

#[tokio::test]
async fn sweep_reclaims_a_draft_past_its_ttl_and_spares_an_attached_file_of_the_same_age()
-> Result<()> {
    let server = TestServer::start().await?;
    let account = server.account("gc_ttl", Some("admin")).await?;
    let files = &server.ctx.srv().file;

    let draft = upload(&server, account.id, "draft.txt", b"a").await?;
    let attached = upload(&server, account.id, "attached.txt", b"b").await?;
    files.attach(account.id, attached.file.id).await?;

    backdate_row(&server, draft.file.id, AGE).await?;
    backdate_row(&server, attached.file.id, AGE).await?;

    assert_eq!(
        files.sweep_unattached(TTL).await?,
        1,
        "the age is equal, so only the unattached one is reclaimable"
    );

    let draft_id = draft.file.id;
    assert!(files.find_by_id(draft_id).await?.is_none());
    assert!(server.ctx.media().open(draft_id).await?.is_none());

    let attached_id = attached.file.id;
    assert!(files.find_by_id(attached_id).await?.is_some());
    assert!(server.ctx.media().open(attached_id).await?.is_some());

    Ok(())
}

#[tokio::test]
async fn sweep_keeps_a_draft_younger_than_its_ttl() -> Result<()> {
    let server = TestServer::start().await?;
    let account = server.account("gc_fresh", Some("admin")).await?;
    let files = &server.ctx.srv().file;

    let draft = upload(&server, account.id, "fresh.txt", b"a").await?;

    assert_eq!(files.sweep_unattached(TTL).await?, 0);
    assert!(files.find_by_id(draft.file.id).await?.is_some());
    assert!(server.ctx.media().open(draft.file.id).await?.is_some());

    Ok(())
}

#[tokio::test]
async fn reconcile_deletes_a_stale_orphan_and_keeps_a_fresh_one() -> Result<()> {
    let server = TestServer::start().await?;

    let stale = FileId::new();
    server.ctx.media().write(stale, b"orphan").await?;
    backdate_blob(&server, stale, AGE)?;

    let fresh = FileId::new();
    server.ctx.media().write(fresh, b"orphan").await?;

    let report = server.ctx.srv().file.reconcile(TTL).await?;
    assert_eq!(
        report,
        ReconcileReport {
            orphans_deleted: 1,
            ..Default::default()
        },
        "only the orphan past its grace is deleted"
    );
    assert!(server.ctx.media().open(stale).await?.is_none());
    assert!(server.ctx.media().open(fresh).await?.is_some());

    Ok(())
}

#[tokio::test]
async fn reconcile_reports_a_size_mismatch_without_touching_the_blob() -> Result<()> {
    let server = TestServer::start().await?;
    let account = server.account("gc_size", Some("admin")).await?;
    let files = &server.ctx.srv().file;

    let uploaded = upload(&server, account.id, "a.txt", b"aaa").await?;
    server
        .ctx
        .media()
        .write(uploaded.file.id, b"longer than the row claims")
        .await?;

    let report = files.reconcile(TTL).await?;
    assert_eq!(
        report,
        ReconcileReport {
            size_mismatches: 1,
            ..Default::default()
        }
    );
    assert!(
        files.find_by_id(uploaded.file.id).await?.is_some(),
        "a row whose blob disagrees is reported, not deleted"
    );
    assert!(server.ctx.media().open(uploaded.file.id).await?.is_some());

    Ok(())
}

#[tokio::test]
async fn reconcile_prunes_stale_derivatives_and_keeps_current_ones() -> Result<()> {
    let server = TestServer::start().await?;
    let account = server.account("gc_derivatives", Some("admin")).await?;
    let files = &server.ctx.srv().file;
    let media = server.ctx.media();

    let uploaded = upload(&server, account.id, "a.txt", b"aaa").await?;
    let id = uploaded.file.id;

    for name in ["v1-256.avif", "v1-256.jpg"] {
        media.write_thumb(id, name, b"keep").await?;
    }
    media.write_thumb(id, "v0-256.avif", b"stale").await?;
    media
        .write_thumb(id, ".v1-256.avif.leftover.tmp", b"junk")
        .await?;

    let report = files.reconcile(TTL).await?;
    assert_eq!(
        report,
        ReconcileReport {
            derivatives_deleted: 2,
            ..Default::default()
        },
        "only the names outside the current pipeline set go"
    );

    let remaining = media.list_derivatives(id).await?;
    assert!(
        remaining.contains(&"v1-256.avif".to_string()),
        "{remaining:?}"
    );
    assert!(
        remaining.contains(&"v1-256.jpg".to_string()),
        "{remaining:?}"
    );
    assert!(
        !remaining.contains(&"v0-256.avif".to_string()),
        "{remaining:?}"
    );
    assert!(
        !remaining.contains(&".v1-256.avif.leftover.tmp".to_string()),
        "{remaining:?}"
    );

    assert!(
        files.find_by_id(id).await?.is_some(),
        "the row is untouched"
    );
    assert!(media.open(id).await?.is_some(), "the origin is untouched");

    Ok(())
}

#[tokio::test]
async fn reconcile_reports_a_row_without_its_blob() -> Result<()> {
    let server = TestServer::start().await?;
    let account = server.account("gc_missing_blob", Some("admin")).await?;
    let files = &server.ctx.srv().file;

    let uploaded = upload(&server, account.id, "a.txt", b"aaa").await?;
    server.ctx.media().delete(uploaded.file.id).await?;

    let report = files.reconcile(TTL).await?;
    assert_eq!(
        report,
        ReconcileReport {
            missing_blobs: 1,
            ..Default::default()
        }
    );
    assert!(
        files.find_by_id(uploaded.file.id).await?.is_some(),
        "a dangling row is reported, not deleted"
    );

    Ok(())
}

#[tokio::test]
async fn deleting_a_user_removes_its_rows_and_leaves_the_blobs_to_gc() -> Result<()> {
    let server = TestServer::start().await?;
    let account = server.account("gc_user_delete", Some("admin")).await?;
    let files = &server.ctx.srv().file;

    let uploaded = upload(&server, account.id, "a.txt", b"aaa").await?;
    let id = uploaded.file.id;

    server.ctx.srv().user.delete(account.id).await?;

    assert!(
        files.find_by_id(id).await?.is_none(),
        "the row goes with the user"
    );
    assert!(
        server.ctx.media().open(id).await?.is_some(),
        "the blob stays for the reconcile pass"
    );

    backdate_blob(&server, id, AGE)?;
    let report = files.reconcile(TTL).await?;
    assert_eq!(
        report,
        ReconcileReport {
            orphans_deleted: 1,
            ..Default::default()
        }
    );
    assert!(server.ctx.media().open(id).await?.is_none());

    Ok(())
}
