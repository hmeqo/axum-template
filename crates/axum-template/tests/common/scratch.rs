//! 集成测试的数据库：每个 [`Scratch`] 一个独占库，建库时就应用 migrations 并装配默认角色，
//! 随 `Drop` 连同临时媒体目录一起删除。
//!
//! 进程被强杀（或置了 `AXUM_TESTS_KEEP_SCRATCH`）时库会留下，名字形如
//! `<db>_scratch_<test-binary>_<pid>_<n>`。

use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use axum_template::{
    app::AppContext,
    config::{AppConfig, ConfigData},
    domain::service::bootstrap::init_rbac,
};
use tokio_postgres::{Client, Config, NoTls};

/// 置位则保留独占库与媒体目录，供事后排查（打印的名字要 `cargo test -- --nocapture` 才可见）。
const KEEP_ENV: &str = "AXUM_TESTS_KEEP_SCRATCH";

const CONNECT_HINT: &str = "integration tests need a working PostgreSQL \
                            (see AGENTS.md: Testing & QA)";

/// 本进程独占的库与媒体目录。
pub struct Scratch {
    base_url: String,
    name: String,
    database_url: String,
    media_dir: PathBuf,
}

impl Scratch {
    /// 建一个可用的独占库：建库 → 应用 migrations → 装配默认角色；再备好媒体目录。
    ///
    /// 中途失败（或调用方随后失败）由 `Drop` 收拾，不会留下半成品。
    pub async fn prepare(base: &AppConfig) -> Result<Self> {
        let base_url = base.database.url.clone();
        let tag = target_name()?;
        let name = scratch_name(&base_url, &tag);

        let scratch = Self {
            database_url: replace_dbname(&base_url, &name)?,
            base_url,
            name,
            media_dir: media_dir(&tag),
        };

        recreate_database(&scratch.base_url, &scratch.name).await?;
        apply_migrations(&scratch.database_url).await?;
        seed_rbac(base, &scratch.database_url).await?;
        reset_dir(&scratch.media_dir)?;

        Ok(scratch)
    }

    /// 把独占库与媒体目录写进配置。
    pub fn apply(&self, config: &mut ConfigData) {
        config.database.url = self.database_url.clone();
        config.storage.media_dir = self.media_dir.to_string_lossy().into_owned();
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if std::env::var_os(KEEP_ENV).is_some() {
            // libtest 会连子线程的输出一起捕获，故这条提示只在 `cargo test -- --nocapture` 下可见；
            // 库名本身可推导（`<db>_scratch_<测试二进制>_<pid>_<n>`），不依赖它也能找到。
            eprintln!("scratch kept: {} / {}", self.name, self.media_dir.display());
            return;
        }

        let base_url = self.base_url.clone();
        let name = self.name.clone();
        let media_dir = std::mem::take(&mut self.media_dir);

        // `Drop` 不能 await：回收交给一次性 runtime，并 join 住，保证进程不会先退出。
        let _ = std::thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                eprintln!("scratch database {} not dropped: no cleanup runtime", name);
                return;
            };

            if let Err(err) = runtime.block_on(drop_database(&base_url, &name)) {
                eprintln!("scratch database {name} not dropped: {err:#}");
            }
            let _ = std::fs::remove_dir_all(&media_dir);
        })
        .join();
    }
}

async fn recreate_database(base_url: &str, name: &str) -> Result<()> {
    let (client, connection) = admin_client(base_url).await?;
    tokio::spawn(connection);

    drop_database_on(&client, name).await?;
    client
        .batch_execute(&format!("CREATE DATABASE \"{name}\""))
        .await?;

    Ok(())
}

async fn apply_migrations(url: &str) -> Result<()> {
    let config = Config::from_str(url).context("database.url could not be parsed")?;
    let (client, connection) = config.connect(NoTls).await.context(CONNECT_HINT)?;
    tokio::spawn(connection);

    for file in migrations()? {
        let sql = std::fs::read_to_string(&file)
            .with_context(|| format!("failed to read migration {}", file.display()))?;
        client
            .batch_execute(&sql)
            .await
            .with_context(|| format!("failed to apply migration {}", file.display()))?;
    }

    Ok(())
}

/// 默认角色随库一起建好。
async fn seed_rbac(base: &AppConfig, url: &str) -> Result<()> {
    let media_dir = media_dir("seed");
    let mut config = ConfigData::clone(base);
    config.database.url = url.to_owned();
    config.storage.media_dir = media_dir.to_string_lossy().into_owned();

    let ctx = AppContext::new(AppConfig::new(config)?).await?;
    let seeded = init_rbac(ctx.srv()).await;
    drop(ctx);

    let _ = std::fs::remove_dir_all(&media_dir);
    seeded?;
    Ok(())
}

async fn drop_database(base_url: &str, name: &str) -> Result<()> {
    let (client, connection) = admin_client(base_url).await?;
    tokio::spawn(connection);

    drop_database_on(&client, name).await
}

/// 踢掉残留连接后删库。
async fn drop_database_on(client: &Client, name: &str) -> Result<()> {
    // 连接池可能刚重连过，`DROP` 因此重试。
    let mut last = None;
    for _ in 0..3 {
        terminate_backends(client, name).await?;
        match client
            .batch_execute(&format!("DROP DATABASE IF EXISTS \"{name}\""))
            .await
        {
            Ok(()) => return Ok(()),
            Err(err) => {
                last = Some(err);
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }

    Err(last.expect("the loop runs at least once").into())
}

/// 踢掉目标库上的其它会话。
async fn terminate_backends(client: &Client, name: &str) -> Result<()> {
    client
        .execute(
            "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
             WHERE datname = $1 AND pid <> pg_backend_pid()",
            &[&name],
        )
        .await?;

    Ok(())
}

/// 建库/删库都要连到 maintenance 库。
async fn admin_client(
    base_url: &str,
) -> Result<(
    Client,
    tokio_postgres::Connection<tokio_postgres::Socket, tokio_postgres::tls::NoTlsStream>,
)> {
    let mut config = Config::from_str(base_url).context("database.url could not be parsed")?;
    config.dbname("postgres");

    config.connect(NoTls).await.context(CONNECT_HINT)
}

/// Postgres 的标识符上限是 63 字节；超出时截掉命名前缀，唯一的尾部必须留下。
fn fit_name(name: &str) -> String {
    const MAX: usize = 63;

    name.char_indices()
        .find(|(index, _)| name.len() - index <= MAX)
        .map(|(index, _)| &name[index..])
        .unwrap_or(name)
        .to_owned()
}

/// `<db>_scratch_<test-binary>_<pid>_<n>`。
fn scratch_name(base_url: &str, tag: &str) -> String {
    static SEQ: AtomicU32 = AtomicU32::new(0);

    let head = format!(
        "{}_scratch_{tag}",
        dbname(base_url).unwrap_or_else(|_| "db".to_owned())
    );
    let suffix = format!(
        "_{}_{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    );

    fit_name(&format!("{head}{suffix}"))
}

fn media_dir(tag: &str) -> PathBuf {
    static SEQ: AtomicU32 = AtomicU32::new(0);

    std::env::temp_dir().join(format!(
        "axum-template-media-{tag}-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ))
}

fn reset_dir(dir: &Path) -> Result<()> {
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    std::fs::create_dir_all(dir)?;

    Ok(())
}

/// 测试二进制名：cargo 会加 `-<16 位哈希>` 后缀。
fn target_name() -> Result<String> {
    let exe = std::env::current_exe()?;
    let stem = exe
        .file_stem()
        .context("test binary should have a file name")?
        .to_string_lossy()
        .into_owned();

    Ok(sanitize(strip_cargo_hash(&stem)))
}

fn strip_cargo_hash(stem: &str) -> &str {
    match stem.rsplit_once('-') {
        Some((head, hash)) if hash.len() == 16 && hash.chars().all(|c| c.is_ascii_hexdigit()) => {
            head
        }
        _ => stem,
    }
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn dbname(url: &str) -> Result<String> {
    let (_, rest) = url
        .split_once("://")
        .context("database.url could not be parsed")?;
    let (_, tail) = rest
        .split_once('/')
        .context("database.url has no database name")?;
    let path = tail.split('?').next().unwrap_or_default();

    if path.is_empty() {
        bail!("database.url has no database name: {url:?}");
    }

    Ok(path.to_owned())
}

fn replace_dbname(url: &str, name: &str) -> Result<String> {
    let Some((scheme, rest)) = url.split_once("://") else {
        bail!("tests need a database.url of the form postgres://, got {url:?}");
    };
    let (authority, tail) = rest.split_once('/').unwrap_or((rest, ""));
    let query = tail
        .split_once('?')
        .map(|(_, query)| format!("?{query}"))
        .unwrap_or_default();

    Ok(format!("{scheme}://{authority}/{name}{query}"))
}

fn migrations() -> Result<Vec<PathBuf>> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../toasty/migrations");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .with_context(|| format!("failed to read migrations directory {}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "sql"))
        .collect();
    files.sort();

    if files.is_empty() {
        bail!("no migration files in {}", dir.display());
    }

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scratch_url_keeps_everything_but_the_database() {
        assert_eq!(
            dbname("postgres://postgres:@localhost:5432/db").unwrap(),
            "db"
        );
        assert_eq!(
            dbname("postgres://u:p@h:5432/db?sslmode=disable").unwrap(),
            "db"
        );
        assert!(dbname("postgres://u:p@h:5432").is_err());

        assert_eq!(
            replace_dbname("postgres://postgres:@localhost:5432/db", "db_scratch_x").unwrap(),
            "postgres://postgres:@localhost:5432/db_scratch_x"
        );
        assert_eq!(
            replace_dbname("postgres://u:p@h:5432/db?sslmode=prefer", "t").unwrap(),
            "postgres://u:p@h:5432/t?sslmode=prefer"
        );
        assert!(replace_dbname("host=localhost dbname=db", "t").is_err());
    }

    #[test]
    fn cargo_hash_leaves_the_database_name_stable() {
        assert_eq!(
            strip_cargo_hash("routing_test-216996d1ca9fd2c6"),
            "routing_test"
        );
        assert_eq!(strip_cargo_hash("routing_test"), "routing_test");
        assert_eq!(strip_cargo_hash("a-deadbeef"), "a-deadbeef");
        assert_eq!(sanitize("a-b"), "a_b");
    }

    #[test]
    fn a_long_name_keeps_its_unique_tail() {
        let long = format!("{}{}", "x".repeat(80), "_unique");
        let fitted = fit_name(&long);

        assert_eq!(fitted.len(), 63);
        assert!(fitted.ends_with("_unique"));
    }

    #[test]
    fn a_multibyte_name_keeps_its_unique_tail_within_the_byte_limit() {
        let long = format!("{}_unique", "库".repeat(40));
        let fitted = fit_name(&long);

        assert!(fitted.len() <= 63, "{} bytes: {fitted}", fitted.len());
        assert!(fitted.ends_with("_unique"));
    }
}
