# Testing and QA

Depth for `AGENTS.md` → *Testing & QA*. The rules ("what to pin, what never to pin") are in `AGENTS.md`;
this file carries what running the suite takes and what the harness cannot say by itself.

## Running

```bash
cargo test --test files_test a_raster_original_is_inlined_and_does_not_vary_on_accept   # one test
AXUM_TESTS_KEEP_SCRATCH=1 cargo test -- --nocapture   # keep scratch DB + media dir, print their names
AXUM_TEMPLATE__DATABASE__URL=postgres://user:pw@host:5432/db cargo test   # override the server
```

- The configured `database.url` must be reachable; the harness connects to it as the **admin** URL (the
  database name is forced to `postgres`) and creates its own scratch databases there. **Never point a
  test run at a shared or production cluster** — every `TestServer::start()` issues `CREATE DATABASE`
  and `DROP DATABASE`.
- No TTY, no prompts. Each test starts its own server, so wall time scales with the test count rather
  than with TTLs; there are no sleeps for ages.

## The harness

`crates/axum-template/tests/common/` is reached with `mod common;` and re-exported through its barrel:
`TestServer`, `TestRequest`, `TestResponse`, `Scratch`, `Account`, `Session`, `Socket`, `in_secs`,
`login_body` and the WS helpers. The API documents itself on the items — `test_server.ctx` for state,
`login_with_headers` for a client fingerprint, `age_session` for time, `ws_without_session_is_rejected`
instead of a hand-rolled handshake assertion. Hover, or `cargo doc --document-private-items`.

Each `*_test.rs` opens with a `//!` naming the contract it pins; that line plus the assertion names is
the per-file index. `tests/common/mod.rs::PUBLIC_PATHS` is the single list of paths that need no
credential: `route_guard_test` skips those when it checks every documented route answers 401, and
`openapi_test` requires every other operation to declare its credentials. A route that becomes public is
one line there.

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

## Unit tests

They sit next to their subject — a `#[cfg(test)]` module in the file it covers, plus the harness's own
tests in `tests/common/scratch.rs` — and they pin the pure logic: status/code tables,
content-disposition encoding and `If-None-Match`, `Accept` negotiation, the frame contract, config
validation and origin parsing, proxy resolution, session expiry, the thumbnail pipeline, credential
bounds, pagination windows, `Perm` wire values, hub/socket lifecycle, storage naming and limits.

## What the suite deliberately does not pin

Wiring, module layout, incidental defaults, framework behavior and message wording: every assertion
reads observable HTTP/DB/WS state. Status + business `code` pairs are asserted, not prose.
`route_guard_test` derives its inventory from the published document, so a new route is covered without
editing the test. `thumb_names()` is pinned to the exact ten-name set, and a rejected upload leaves
zero rows and no blob — invariants, not implementation.
