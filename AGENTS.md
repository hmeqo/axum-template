# AGENTS.md

## Project

Rust Axum web API template. Workspace with one crate: `crates/backend`.

## Build & run

```bash
cargo check                          # check (also checks tests)
cargo run --bin backend              # start server
cargo run --bin toasty-cli -- migration apply   # run pending migrations
cargo run --bin toasty-cli -- migration generate --name xxx  # diff models → SQL
./manage.sh dev                      # cargo watch hot-reload
```

Config is read from `.axum-template/config.toml` (or XDG config dir).  
Default database: `postgres://postgres:@localhost:5432/db`.

## Database

- **ORM**: Toasty (`toasty::Model` derive). Models in `domain/model/`.
- **Migrations**: `toasty-cli` binary. Schema managed by migration files.
- **Initial data**: `cargo run --bin backend -- init` creates default roles/permissions.
- **Connection**: `domain/db/connection.rs::init_db()`.
- **Model registration**: Any new Toasty model must be added to:
  - `domain/db/connection.rs` (model list)
  - `src/bin/toasty-cli.rs` (model list)

## Auth — two coexisting mechanisms

### Session auth (default for `/api/auth/*`)

```
POST /api/auth/login   → sets cookie, returns user+permissions
GET  /api/auth/me      → SessionCtx → user+permissions
POST /api/auth/logout  → deletes session
```

- Sessions stored in `sessions` table, identified by `session_token` cookie.
- TTL auto-extended on every request via `SessionCtx` extractor.
- `SessionCtx` in `app/helper/auth/session.rs`.

### JWT auth (example at `/api/auth/jwt/*`)

```
POST /api/auth/jwt/login   → returns access_token + refresh_token
POST /api/auth/jwt/refresh → rotates token pair
POST /api/auth/jwt/logout  → deletes refresh tokens
GET  /api/auth/jwt/me      → JwtCtx → user+permissions
GET  /api/auth/jwt/echo    → JwtCtx → {user_id, username}
```

- `access_token` (JWT, configurable expiry) + `refresh_token` (UUID, 30d, stored in `refresh_tokens` table).
- `JwtCtx` extractor in `app/helper/auth/jwt.rs` decodes JWT from `Authorization: Bearer <token>`.
- JWT secret: `config.auth.jwt.secret` (default `"change-me-in-production"`).

## Architecture

```
Controller (axum handler)
  → state.srv().user.xxx() / .role.xxx() / .token.xxx() / .session.xxx()
    → Service struct (holds cloned toasty::Db + relevant config)
      → Toasty API (model queries)
```

- **No repository layer** — services call Toasty directly.
- `Services` struct in `domain/mod.rs` includes: `user`, `role`, `permission`, `auth`, `session`, `token`.

## Error handling

- `ErrorKind` enum in `error.rs`. Builder methods: `.msg()`, `.err()`, `.err_msg()`.
- `ResultExt` trait: `.err_kind(ErrorKind)` / `.err_kind_msg(ErrorKind, msg)`.
- `OptionAppExt` trait: `.ok_or_err()` / `.ok_or_err_msg()`.
- `bail!(kind, msg)` macro for early returns.
- Only `Config` / `Internal` errors log to tracing (business errors = 4xx, no log noise).
- `From<toasty::Error> for AppError` is registered — `?` works on Toasty calls.

## DTO validation

`AppJson`, `AppQuery`, `AppPath` extractors auto-validate (`#[validate(...)]`). Validation errors return `400` with structured `errors` field.

## Migrations workflow

1. Edit models in `domain/model/`
2. `./manage.sh migration-generate --name describe_change`
3. Review generated SQL in `toasty/migrations/`
4. `./manage.sh migrate`
5. Commit migration files alongside model changes

## Tests

Integration tests in `tests/integration_test.rs`.

```rust
let config = AppConfig::load()?;
let app_state = AppState::from_config(config).await?;
let app = create_router(app_state).await?;
```

## Key files

| Path | Purpose |
|------|---------|
| `domain/mod.rs` | `Services` DI (user, role, permission, auth, session, token) |
| `domain/model/` | Toasty models |
| `domain/service/` | Stateless service structs |
| `domain/service/token.rs` | TokenService (JWT encode/decode, refresh token CRUD) |
| `domain/service/session.rs` | SessionService (create/find/extend/delete sessions) |
| `app/serve.rs` | `serve()` + `create_app()` entrypoints |
| `app/helper/auth/mod.rs` | `JwtCtx` + `SessionCtx` extractors (dir module) |
| `app/helper/auth/jwt.rs` | `JwtCtx` (Bearer token extractor) |
| `app/helper/auth/session.rs` | `SessionCtx` (cookie extractor) + cookie helpers |
| `app/helper/extractor.rs` | AppJson, AppQuery, AppPath validation extractors |
| `app/error.rs` | `IntoResponse` for `AppError`, status code mapping |
| `app/controller/auth.rs` | Session auth handlers (login/me/logout) |
| `app/controller/jwt_demo.rs` | JWT auth example handlers |
| `config/schema.rs` | AppConfig struct with Default impls |
| `config/env.rs` | Environment variable helpers |
| `config/meta.rs` | Project constants |
| `config/paths.rs` | Path resolution (config file, data dir) |
