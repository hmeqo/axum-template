# Testing and QA

Depth for `AGENTS.md` → *Testing & QA*. The rules ("what to pin, what never to pin") are in `AGENTS.md`;
this file is the harness reference and the per-file contract table.

## Running

```bash
cargo test                                   # full suite; needs a reachable PostgreSQL
cargo test --features webtransport           # adds tests/webtransport_test.rs (feature-gated file)
cargo test --test files_test a_raster_original_is_inlined_and_does_not_vary_on_accept
AXUM_TESTS_KEEP_SCRATCH=1 cargo test -- --nocapture   # keep scratch DB + media dir, print their names
AXUM_TEMPLATE__DATABASE__URL=postgres://user:pw@host:5432/db cargo test   # override the server
```

- The configured `database.url` must be reachable; the harness connects to it as the **admin** URL (the
  database name is forced to `postgres`) and creates its own scratch databases there. **Never point a
  test run at a shared or production cluster** — every `TestServer::start()` issues `CREATE DATABASE`
  and `DROP DATABASE`.
- No TTY, no prompts. Each test starts its own server, so wall time scales with the test count rather
  than with TTLs; there are no sleeps for ages (see below).

## The harness (`crates/axum-template/tests/common/`)

Every `*_test.rs` opens with `mod common;` and imports what it needs. The barrel re-exports
`TestServer`, `TestRequest`, `TestResponse`, `Scratch`, `Account`, `Session`, `Socket`, `in_secs`,
`login_body` and the WS helpers.

| Item | Behavior |
|---|---|
| `TestServer::start()` / `start_with(|config| …)` | loads the real config, points it at a scratch database and temp media dir, seeds the default roles, mounts the real router on `127.0.0.1:0`. `start_with` is the hook for config mutations (e.g. clearing `trusted_proxies`) |
| `server.ctx` | the live `AppContext` — `ctx.srv()`, `ctx.cfg()`, `ctx.media()` are how a test inspects state |
| `server.get/post/put/delete(uri)` | a `TestRequest`; also `server.request(Method, uri)`. Builders: `.json(&body)` (a **reference**), `.body(content_type, impl Into<Bytes>)`, `.cookie(&Session)`, `.raw_cookie(&str)`, `.bearer(&token)`, `.header(name, value)` |
| `.send().await?` | runs the request through `oneshot` on the router (no listener) and buffers the body |
| `TestResponse` | `status`, `json()` (non-JSON/empty → `null`), `text()`, `bytes()`, `header(name)`, `header_values(name)`, `set_cookie()`, `set_cookies()`, `code()`, `extension::<T>()` |
| `expect(status)` / `ok()` / `expect_code("auth.invalid_credentials")` | `#[track_caller]` assertions that panic with the request line, status and body — prefer them over hand-written `assert_eq!` |
| `TestServer::account(name, role)` / `login(name, role)` | create a user (optionally with a role) / create + log in; `Account::PASSWORD` is `"password123"`, usernames are unique per process |
| `login_with_headers(&account, &[(HeaderName, &str)])` | log in with extra headers (User-Agent, `X-Forwarded-For` fingerprints) |
| `open_session`, `access_token(&account)` | session from an existing account; an access token from `POST /api/auth/jwt/login` |
| `server.age_session(&token, expires_at)` | rewrites the row's `expires_at` through the ORM — how time-dependent behavior is tested without sleeping |
| `in_secs(offset)` | a `jiff::Timestamp` truncated to whole seconds (Postgres round-trips micros) |
| `server.openapi()` | `GET /api-docs/openapi.json` as a `serde_json::Value` |
| `server.ws(path, Option<&Session>)`, `ws_without_session_is_rejected(path)` | a real WebSocket to `server.addr` (requests go through `oneshot`, so WS tests must use the listener); the second asserts a 401 `tungstenite::Error::Http` |
| `send`, `next_frame`, `close_frame`, `wait_for` | WS helpers: 5 s frame timeout, `close_frame` returns `(code, reason)`, `wait_for` scans at most 16 frames |
| `Scratch::prepare(&config)` / `apply(&mut config)` | build a scratch database + media dir yourself (used by `webtransport_test.rs`, which needs its own harness) |

## Isolation and determinism

- Every `TestServer` owns a database named `<db>_scratch_<tag>_<pid>_<n>` and a media directory
  `axum-template-media-<tag>-<pid>-<n>` under the system temp dir; `<tag>` is the test-binary stem
  (cargo's `-<16 hex>` suffix stripped). The name is byte-truncated to Postgres' 63-character limit by
  dropping the prefix and keeping the unique tail. Creation = drop-if-exists → `CREATE DATABASE` → apply
  every `toasty/migrations/*.sql` in filename order → seed RBAC with a real `AppContext`.
- Both are dropped when the `TestServer` is (the drop runs on a one-shot runtime that is joined, so a
  normal run leaves nothing behind). `AXUM_TESTS_KEEP_SCRATCH=1` keeps them and prints the names.
- A **killed** process leaves its database and media dir behind, and the next run does not reclaim them
  (the next run only drops the name derived from *its own* pid) — drop leftovers by name yourself.
- Time is forged, never slept: `age_session` rewrites `expires_at`; `file_gc_test` backdates
  `files.created_at` with raw SQL and sets the blob's mtime on disk; `realtime_test` has one 300 ms
  negative-assertion window; `webtransport_test` retries for up to 2 s while the UDP port finishes
  binding. A test that needs a real database connection uses
  `tokio_postgres::connect(&server.ctx.cfg().database.url, NoTls)` to count rows, backdate them or
  assert a CHECK constraint.

## Per-file contract table

| File | Pins |
|---|---|
| `routing_test.rs` | JSON 404 for unknown `/api/*`, SPA fallback for unmatched paths, `no-store` only on `/api`, both probes, missing row → 404 |
| `openapi_test.rs` | every `operationId` unique and `<firstTag>__…`; `ErrorCode` / `Perm` / `FileStateCode` enumerations equal `iter()`; `ErrorResp` reachable under a concrete status; every `$ref` resolves; no CJK in `openapi.json` or the proto contract |
| `route_guard_test.rs` | every documented route answers 401 without a session (except the file-local `PUBLIC` list) and so do the two undocumented `/api/rpc` method paths; the route inventory is derived from the live document |
| `error_resp_test.rs` | `{code?, message, errors?}` shape; transport failures carry no code; malformed body → codeless 400; a JSON body over the body limit → 413 `file.too_large`; `x-request-id` on `/api` and the media route, absent on `/health`; header equals the `RequestId` extension; `/api/error` serves its body with 200 |
| `session_auth_test.rs` | cookie login/logout (row deleted, cookie cleared), the implicit slide (past half a TTL: window pushed and `Max-Age` matches the row's remaining seconds ±2; more than half: no write, no cookie), logout past half a TTL writes only the clearing cookie |
| `session_devices_test.rs` | the session list records `ip`/`user_agent`/`created_at`/`expires_at` and flags the current device; revoking one device leaves the other; another user's session id → 404; a slide moves `expires_at`; a password change rotates this device's cookie, kills the others (sessions + refresh tokens + their sockets with `(1000, "session revoked")`); `revoke-others` keeps only this device |
| `jwt_auth_test.rs` | access token on `/me`; refresh rotates the pair and a consumed token → `401 auth.refresh_token_invalid`; two concurrent refreshes → exactly one 200; logout invalidates refresh tokens |
| `auth_policy_test.rs` | a resource group and the self-service password change accept either credential; session-only and JWT-only groups reject the other; an invalid `Bearer` is never downgraded to a valid cookie; an expired cookie is `auth.session_expired` and does not block a new login; the challenge (`Bearer realm="api"`, `+ error="invalid_token"`) appears only where `Bearer` is accepted |
| `permission_test.rs` | authn vs authz through real logins; only `user:reset-password` may reset another user's password (and that revokes the target everywhere); a `403` precedes body/path validation (`400`); a token identity passes the same gates |
| `credential_policy_test.rs` | rename, change-password and reset-password reuse the create-time policy (field-level `400 input.invalid_fields`); changing a password requires the old one and has no side effect when it fails |
| `users_filter_test.rs` | `?username=` filters page **and** total, words AND-ed, case-insensitive, order-free; `%`/`_` literal; blank = no filter |
| `user_crud_test.rs` | create → fetch → delete → 404 |
| `pagination_test.rs` | defaults `page=1`/`per_page=10`; the last page inside the `MAX_OFFSET` window is a legal empty page, one past it → codeless 400; `page=0` / `per_page=0` / `per_page=101` → field-level 400; `page=abc` → codeless 400 |
| `files_test.rs` | both 413 sites (`DefaultBodyLimit`, `MediaStore`) and no row/blob on a rejected upload; draft `no-store` with no `ETag` and never 304, attached `immutable` + sha256 `ETag` + 304; the RFC 8187 filename header (non-ASCII, ASCII-only, no CR/LF); a raster original is served `inline` with its filename while other types stay `attachment`; the derivative contract (`?size=` generates then serves from disk, `Accept` switches AVIF/JPEG with distinct validators, never enlarged, `Vary`/`Content-Length`/`inline`); attach authz before any write; own-files-only paginated list; dangling row → 500; the `files_state_attached_at_consistent` CHECK rejects inconsistent state |
| `file_gc_test.rs` | draft sweep (stale reclaimed with its directory, fresh kept, attached untouched) and reconcile (stale orphan directory deleted, fresh kept, size mismatch and missing origin reported but not deleted, derivatives outside the current pipeline set pruned); ages are forged in the DB/on disk |
| `realtime_test.rs` | `/ws` without a session → 401; per-topic online counts and broadcast; unsubscribe stops delivery to that socket only; unknown topic → `realtime.topic_forbidden`; unknown frame → `realtime.unknown_frame` without dropping the connection; `ping` → `pong`; logout, single-session revoke and deleting the user close exactly the affected socket with `(1000, "session revoked")`; a drain closes with `1001`; a disconnect lowers the count |
| `rpc_test.rs` | 401 + `WWW-Authenticate` on `/api/rpc`; `Me` accepts either credential; Connect enforces the same `Require<UserRead>`; `ListUsers` paginates with 64-bit ints as strings and a case-insensitive filter; a rejected page carries a base64 `template.v1.ErrorDetail` with the business code and field errors; the generated client works against the real listener; h2c reflection lists the service |
| `webtransport_test.rs` | CONNECT-header credentials (403 on rejection), roster/join/leave over unidirectional streams, state over datagrams, a revoked session closes the connection (`#![cfg(feature = "webtransport")]`) |

## Unit tests

They sit next to their subject. `#[cfg(test)]` modules exist in `error/mod.rs`, `app/http/error.rs`,
`app/http/response.rs`, `app/identity/mod.rs`, `app/routes/media.rs`, `app/socket/protocol.rs`,
`config/mod.rs`, `config/trusted_proxies.rs`, `domain/model/session.rs`, `domain/service/thumbnail.rs`,
`domain/vo/{credential,file_state,pagination,password_hash,permission,thumb,user_filter}.rs`,
`infra/realtime/{hub,sockets}.rs`, `infra/storage/mod.rs` — plus the harness's own tests in
`tests/common/scratch.rs` (scratch-name derivation and truncation). They pin the pure logic:
status/code tables, content-disposition encoding and `If-None-Match`, `Accept` negotiation, the frame
contract, config validation and origin parsing, proxy resolution, session expiry, the thumbnail
pipeline, credential bounds, pagination windows, `Perm` wire values, hub/socket lifecycle, storage
naming and limits.

## What the suite deliberately does not pin

Wiring, module layout, incidental defaults, framework behavior and message wording: every assertion
reads observable HTTP/DB/WS state. Status + business `code` pairs are asserted, not prose.
`route_guard_test` derives its inventory from the published document, so a new route is covered without
editing the test. `thumb_names()` is pinned to the exact ten-name set, and a rejected upload leaves
zero rows and no blob — invariants, not implementation.

Document-specific rules worth repeating before touching anything published: `operationId`s are unique and
tag-prefixed; the published enums must equal their `iter()` serialization (`Perm` because `ToSchema`
cannot see the macro-interpolated `serde(rename)`); `ErrorResp` must stay reachable under a concrete
status because generators that drop the `4XX`/`5XX` wildcards would emit no error type at all; no
dangling `$ref` (an `IntoResponses` entry contributes no component); and no CJK anywhere in
`openapi.json` or the proto contract.
