# Axum Template

Rust Axum web API template with Toasty ORM, session & JWT auth, RBAC.

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

## Auth — two mechanisms

| Endpoint                     | Auth    | Description                                     |
| ---------------------------- | ------- | ----------------------------------------------- |
| `POST /api/auth/login`       | session | Login via username/password, sets cookie        |
| `GET /api/auth/me`           | session | Current user info (cookie)                      |
| `POST /api/auth/logout`      | session | End session                                     |
| `POST /api/auth/jwt/login`   | JWT     | Login, returns `access_token` + `refresh_token` |
| `POST /api/auth/jwt/refresh` | JWT     | Rotate tokens                                   |
| `POST /api/auth/jwt/logout`  | JWT     | Revoke refresh tokens                           |
| `GET /api/auth/jwt/me`       | JWT     | Current user info (`Authorization: Bearer`)     |
| `GET /api/auth/jwt/echo`     | JWT     | Auth check example                              |

## Commands

```bash
./manage.sh migrate        # run pending migrations
./manage.sh fresh          # drop & recreate tables
./manage.sh init           # seed default roles/permissions
./manage.sh create-superuser
./manage.sh migration-generate --name xxx   # diff models → SQL
```

## Stack

| Layer      | Choice                                  |
| ---------- | --------------------------------------- |
| Web        | Axum 0.8                                |
| ORM        | Toasty 0.5                              |
| Auth       | Session (cookie) + JWT (`jsonwebtoken`) |
| DB         | PostgreSQL                              |
| Migrations | `toasty-cli`                            |
