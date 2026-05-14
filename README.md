# Axum Template

Rust Axum web API template with Toasty ORM, JWT auth, RBAC.

## Quick start

```bash
# Start PostgreSQL, then:
cargo run --bin toasty-cli -- migration apply   # create tables
cargo run --bin backend -- init                 # seed roles/permissions
cargo run --bin backend -- create-superuser     # create superuser

# Start server
cargo run --bin backend

# Or hot-reload
./manage.sh dev
```

## Auth

| Endpoint | Description |
|----------|-------------|
| `POST /api/auth/login` | Returns `access_token` (1h) + `refresh_token` (30d) |
| `POST /api/auth/refresh` | Exchange refresh token for new pair |
| `GET /api/auth/me` | Current user info (requires `Authorization: Bearer <token>`) |

## Commands

```bash
./manage.sh migrate        # run pending migrations
./manage.sh fresh          # drop & recreate tables
./manage.sh init           # seed default roles/permissions
./manage.sh create-superuser
./manage.sh migration-generate --name xxx   # diff models → SQL
```

## Stack

| Layer | Choice |
|-------|--------|
| Web | Axum 0.8 |
| ORM | Toasty 0.5 |
| Auth | JWT (`jsonwebtoken`) |
| DB | PostgreSQL |
| Migrations | `toasty-cli` |
