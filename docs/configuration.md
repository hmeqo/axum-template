# Configuration and environment

Depth for `AGENTS.md` → *Runtime/Tooling Preferences*. Every key lives in `config/schema.rs`; layering
and validation live in `config/mod.rs`.

## Three mechanisms that must not be conflated

| Mechanism | Meaning |
|---|---|
| `AXUM_TEMPLATE_ENV=<profile>` | profile selector: layers `<config-dir>/config.<profile>.toml` (a **required** file once named). Never branch on the profile at runtime — encode prod behavior as keys in `config.prod.toml` |
| `.env` (dotenvy) | fills *unset* process variables only, so real environment variables always win. Read from the config directory (never the CWD) unless `AXUM_TEMPLATE_ENV_FILE` points elsewhere |
| `AXUM_TEMPLATE__<SECTION>__<KEY>` | config layer 4 only: double underscore separates, keys are lower-cased and kebab-converted, so `AXUM_TEMPLATE__STORAGE__MEDIA_URL_PREFIX` is `storage.media-url-prefix` |

Other environment variables: `AXUM_TEMPLATE_CONFIG_FILE` (a single required config file, replacing
sources 2–3), `AXUM_TEMPLATE_LOCAL_MODE` (force the project-local `.<project>` directory), `RUST_LOG`
(overrides `log.level`; otherwise the filter is `axum_template=<log.level>,tower_http=<log.level>`).

Precedence, later wins:

1. Built-in defaults (`ConfigData::default()`).
2. `<config-dir>/config.toml` — optional.
3. `<config-dir>/config.<AXUM_TEMPLATE_ENV>.toml` — required when the profile is named.
4. `AXUM_TEMPLATE__<SECTION>__<KEY>`.

The env-variable prefix is derived from the crate name (`config/meta.rs`): `axum_template__`. The config
library's environment source also matches single-underscore names, so only the double-underscore form
sets config keys — `AXUM_TEMPLATE_ENV` is *not* a config key.

## Where files live

`config/paths.rs` resolves once into a `OnceLock`:

- If `.axum-template/` exists in the CWD, or `AXUM_TEMPLATE_LOCAL_MODE` is set → config dir = data dir
  = `.axum-template/`.
- Otherwise config dir = `dirs::config_dir()/axum-template`, data dir = `dirs::data_dir()/axum-template`.

Derived paths: `<config-dir>/config.toml`, `<config-dir>/config.<profile>.toml`, `<config-dir>/.env`,
`<data-dir>/logs/access.log` (daily rotation), `<data-dir>/<storage.media-dir>` (a relative media dir is
resolved against the data dir; an absolute one is used as-is).

**Caveat an operator will hit**: the default `.env` location is itself decided by that resolution, and
the resolution is frozen the first time any path is queried — which happens *before* `.env` is read.
Consequently `AXUM_TEMPLATE_LOCAL_MODE` written inside the default `.env` does not change the directory
(it only works for a `.env` named by `AXUM_TEMPLATE_ENV_FILE`, or as a real environment variable).
`paths::set_dirs` is a public override with no caller in the tree; it panics if called after a path has
been resolved.

## What `validate()` refuses

`AppConfig::load()` runs `validate()` (`config/mod.rs`), which fails startup on:

- any positive-integer key that is `0` — one shared private helper `positive`, one message
  (`"{key} must be > 0"`): `server.port`, `webtransport.port` (when enabled), `request-timeout-seconds`,
  `shutdown-timeout-seconds`, `auth.session.ttl-hours`, `auth.jwt.expires-in-seconds`,
  `realtime.channel-capacity`, `storage.max-file-size`, `storage.gc-interval-minutes`,
  `storage.unattached-ttl-hours`, `storage.orphan-grace-minutes`;
- `webtransport.enabled` on a build without the `webtransport` cargo feature;
- `webtransport.cert` and `key` set one without the other;
- an empty `database.url`;
- a malformed `storage.media-url-prefix` (must start with `/`, must not contain `{` or `}` — it is
  interpolated into `Router::route`);
- each `cors.allowed-origins` entry not being `Origin`-shaped (scheme + host, lowercase, no userinfo,
  path, query, fragment or trailing slash);
- `auth.session.same-site = "none"` without `secure = true`.

Trusted proxies are different: `server.trusted-proxies` entries are parsed when `AppConfig::new` builds
the proxy table (`config/trusted_proxies.rs`), so a malformed entry fails there rather than in
`validate()`. An empty list means `X-Forwarded-For` is never trusted.

## Defaults

| Key | Default |
|---|---|
| `server.host` / `server.port` | `0.0.0.0` / `8000` |
| `server.request-timeout-seconds` / `server.shutdown-timeout-seconds` | `30` / `10` |
| `server.trusted-proxies` | `[]` — never trust `X-Forwarded-For` |
| `log.level` | `debug` (`RUST_LOG` wins; rotation and JSON file output are hardcoded) |
| `database.url` | `postgres://postgres:@localhost:5432/db` |
| `auth.session.cookie-name` | `session_id` |
| `auth.session.ttl-hours` | `24` (the only session expiry; no absolute cap) |
| `auth.session.secure` / `same-site` | `false` / `lax` |
| `auth.jwt.secret` | `change-me-in-production` — placeholder, set a real secret in every deployment |
| `auth.jwt.expires-in-seconds` | `900` (refresh tokens live 30 days, `RefreshToken::TTL_HOURS`) |
| `storage.max-file-size` | `10485760` (10 MiB); the request body limit is this `+ 1 MiB` |
| `storage.media-url-prefix` / `media-dir` | `/media` / `media` |
| `storage.gc-interval-minutes` / `unattached-ttl-hours` / `orphan-grace-minutes` | `60` / `24` / `60` |
| `realtime.channel-capacity` | `64` per topic (a lagging subscriber is told how many events it missed) |
| `cors.allowed-origins` | `[]` — no cross-origin access |
| `webtransport.enabled` / `port` | `false` / `4433` (no PEM paths ⇒ a self-signed certificate) |

## Operator gotchas

- **Every CLI subcommand boots the app**: it needs a reachable database and creates the media directory.
  `axum-template config` prints the effective configuration as JSON, `auth.jwt.secret` included — route
  its output accordingly.
- `cargo run --bin axum-template` (no subcommand) is the server; it runs on `server.host:server.port`.
- With an empty CORS allowlist no credentials are sent; a non-empty one mirrors the request's methods
  and headers and sets `allow_credentials(true)` (a `*` origin cannot be combined with credentials), and
  the origins were already shape-validated at load time.
- Media dir and logs come from the same resolution as the config, so "where did the uploads go?" is
  answered by which branch of the path resolution is active (local `.axum-template/` vs XDG).
