# AGENTS.md

## Project

Rust Axum web API template. Workspace with one crate: `crates/backend`.

## Build & run

```bash
cargo check                          # check (also checks tests)
cargo run --bin backend              # start server (reads data/config.toml)
cargo run --bin toasty-cli -- migration apply   # run pending migrations
cargo run --bin toasty-cli -- migration generate --name xxx  # diff models → SQL
./manage.sh dev                      # cargo watch hot-reload
```

## Database

- **ORM**: Toasty (`toasty::Model` derive). Models in `domain/model/`.
- **Migrations**: `toasty-cli` binary. `push_schema` is NOT used — schema managed by migration files.
- **Initial data**: `cargo run --bin backend -- init` creates default roles/permissions.
- **Connection**: `domain/db/connection.rs::init_db()`. Config default: `postgres://postgres:@localhost:5432/db`.
- **Model registration**: Any new Toasty model must be added to:
  - `domain/db/connection.rs` (model list)
  - `src/bin/toasty-cli.rs` (model list)

## Auth

- **JWT** (no sessions, no axum-login). `jsonwebtoken` crate.
- `access_token` (JWT, 1h) + `refresh_token` (UUID, 30d, stored in `refresh_tokens` table).
- `AuthCtx` extractor (`app/helper/auth.rs`) decodes JWT from `Authorization: Bearer <token>`.
- JWT secret: `config.auth.jwt_secret` (default `"change-me-in-production"`), overridable via `AUTH__JWT_SECRET` env var.
- Refresh endpoint: `POST /api/auth/refresh` (single-use, rotates token).
- No `/logout` endpoint — client discards token; revoke by deleting `refresh_tokens` row.

## Architecture

```
Controller (axum handler)
  → state.services().user.xxx() / .role.xxx()
    → Service struct (holds cloned toasty::Db)
      → Toasty API (model queries)
```

- **No repository layer** — services call Toasty directly.
- `Domain` and `Services` structs defined in `domain/mod.rs`.

## Error handling

- `ErrorKind` enum in `error.rs`. Builder methods: `.msg()`, `.err()`, `.err_msg()`.
- `ResultExt` trait: `.err_kind(ErrorKind)` on `Result<T, impl StdError>`.
- `OptionAppExt` trait: `.ok_or_err()`, `.ok_or_err_msg()` on `Option<T>`.
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

Integration tests in `tests/integration_test.rs`. Use `create_router()` directly with inline setup:

```rust
let config = AppConfigManager::default()?;
let domain = Domain::from_config(&config.load()).await?;
let app = create_router(AppState { config, domain }).await?;
```

## Key files

| Path | Purpose |
|------|---------|
| `domain/mod.rs` | `Domain`, `Services` DI |
| `domain/model/` | Toasty models (User, Role, Permission, UserRole, RolePermission, RefreshToken) |
| `domain/service/` | Stateless service structs |
| `app/serve.rs` | `serve()` + `create_app()` entrypoints |
| `app/helper/auth.rs` | JWT Claims, encode/decode, AuthCtx extractor |
| `app/helper/extractor.rs` | AppJson, AppQuery, AppPath validation extractors |
| `app/error.rs` | `IntoResponse` for `AppError`, status code mapping |
| `app/controller/` | Axum handlers |
| `config/schema.rs` | AppConfig struct with Default impls |
