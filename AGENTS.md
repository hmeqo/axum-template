# Repository Guidelines

Agent-facing rules for this repository, plus the index that points at the depth. Keep it that way:
a fact has **one** home — a rule here, or the doc that explains it, never both. Code is the source of
truth; a line in any of these files that contradicts it is a bug in the file.

| File | Carries |
|---|---|
|`AGENTS.md` (this file)|the rules, the directory/file indexes, the commands|
| `docs/architecture.md` | why the code is shaped this way: layering, request path, auth flow, realtime/RPC, file storage, error classification, OpenAPI assembly |
| `docs/development.md` | toolchain, build, dependencies, protobuf, migrations, CLI, local state |
| `docs/configuration.md` | config precedence, environment variables, `validate()`, path resolution, operator gotchas |
| `docs/testing.md` | test harness, how to run, isolation, unit-test layout, what not to pin |

## Project Overview

Rust 2024 (nightly) + Axum 0.8 + Toasty 0.11 (PostgreSQL) reference web API template — one workspace
member, `crates/axum-template`, one library plus two binaries:

| Binary | Entry point | Role |
|---|---|---|
| `axum-template` | `crates/axum-template/src/main.rs` | server when no subcommand; admin CLI (`config`, `init`, `create-superuser`, `role …`, `perms`) |
| `toasty-cli` | `crates/axum-template/src/bin/toasty-cli.rs` | migrations, driven by `manage` and `Toasty.toml` |

Features: session-cookie **and** JWT auth, RBAC over one `Perm` catalogue, per-entity id newtypes,
`Pagination`, upload/download with derivatives, `/ws` realtime sample, a ConnectRPC surface generated
from `proto/` (plus gRPC reflection), optional WebTransport sample, OpenAPI for the REST surface,
health probes, embedded SPA, layered config.

Template, not a framework: copy the patterns and delete what the target service does not need. The flip
side: an unused-but-plausible hook (a CRUD method, a config override, an accessor, a Cargo feature)
stays even when nothing calls it — a copy may need it, so it is not dead weight.

## Architecture & Data Flow

```
HTTP → TraceLayer → CorsLayer → TimeoutLayer(408) → DefaultBodyLimit → error_log
     → /api router: request_id → no_store → matched route → group auth layer → extractors → handler
     → ctx.srv().<service> → Toasty → PostgreSQL
RPC: → /api/rpc (nested Connect service) → group auth layer → dispatcher → app/rpc/service.rs → ctx.srv()
SPA: → static/ (embedded) served for `/` and every unmatched path
```

```
app/       HTTP edge and assembly: context.rs (DI) · startup.rs (boot/drain) · router.rs (only assembly
           point: paths, tags, global middleware order) · http/ (error, response, openapi) · routes/ ·
           dto/ · extractor/ · middleware/ · identity/ · socket/ (long-lived faces) · rpc/ (generated
           Connect service) · event.rs (the realtime bus payload union)
domain/    model/ (Toasty rows) · service/ (use cases) · vo/ (value objects, invariants) ·
           db/ (named query shapes, app_models!, init_db)
infra/     logging/ · scheduler/ (interval tasks) · signal.rs (draining) · storage/ (MediaStore) ·
           realtime/ (Hub, Sockets)
config/    schema.rs (defaults) · mod.rs (layering + validate()) · paths.rs · env.rs · meta.rs ·
           trusted_proxies.rs
cli/       command.rs (clap tree) · run.rs (dispatch) · command_impl.rs
error/     mod.rs (AppError, bail!) · code.rs (ErrorCode) · mapping.rs (foreign From impls only)
```

- **Layers hold no loose files**: `domain/`, `app/routes/`, `app/dto/`, `app/socket/`; each route module
  owns its mount prefix, and a group's auth layer is mounted by the module that owns the group.
- **Persistence is Toasty behind `domain/`** — no repository layer. A query lives in the service that
  owns its aggregate (relations through the model's association paths, never a hand-rolled two-step
  load); a read shape the owning service names rather than writes inline (a paginated list with its
  count, a delete-by-owner pair) is a free function in `domain/db/`.
- **Deletes cascade through the declared relations**: a child whose foreign key is required
  (`#[belongs_to]` + non-`Option` key) is deleted with its parent, an optional one gets its key set to
  `NULL`. Declare `#[index]` + `#[belongs_to]` on the child and `#[has_many]` on the parent — that
  declaration is what makes the cascade happen — then call the generated `delete_by_id`; never hand-write
  the graph or its transaction. New models are registered only in `app_models!` (`domain/db/mod.rs`).
- **Authentication is three jobs in three places**: `AuthBackend` per group (which credentials, in what
  order), `Resolved` in the request extensions (one verdict per request), `Require<P>` (authorization
  only). One credential per request — `Bearer` before the cookie, first *present* one decides, present
  but invalid is a 401 and never downgraded; a group without a layer fails closed (401); 401 always
  precedes 403. Response bodies are `{code?, message, errors?}`; clients branch on `code`.
- **State**: `AppContext` wraps `Arc<AppContextInner>` and derefs to it, so axum's per-request `State`
  clone is a refcount bump — reach services/config/storage through `cfg()`, `db()`, `srv()`, `media()`,
  `events()`, `sockets()`, `shutdown()`.
- Depth for all of the above: `docs/architecture.md`.

### Endpoint surface

The surface is the code, never a list here: a route module's `router()` is its mount and its auth
backend, the `#[utoipa::path]` above a handler is the only place that handler's path, parameters, body
and responses are written, and the extractors in its signature are what a caller must present
(`Auth<B>` identity, `Require<P>` permission) — the `///` above the attribute is the text clients read.
`app/router.rs::create_router` is the whole inventory, `GET /api-docs/openapi.json` its rendered form,
and `tests/route_guard_test.rs` derives its own inventory from that document rather than from a table.
`{storage.media-url-prefix}`, `/ws` and the doc UIs mount outside the `/api` nest.

## Key Directories

| Path | Responsibility |
|---|---|
| `crates/axum-template/src/app/` | HTTP edge and assembly (`context.rs`, `startup.rs`, `router.rs`, `event.rs`, `http/`, `routes/`, `dto/`, `extractor/`, `middleware/`, `identity/`, `socket/`, `rpc/`) |
| `crates/axum-template/src/domain/` | `model/`, `service/`, `vo/`, `db/` |
| `crates/axum-template/src/infra/` | `logging/`, `scheduler/`, `signal.rs`, `storage/`, `realtime/` |
| `crates/axum-template/src/config/` | `schema.rs`, `mod.rs`, `paths.rs`, `env.rs`, `meta.rs`, `trusted_proxies.rs` |
| `crates/axum-template/src/cli/` | `command.rs`, `run.rs`, `command_impl.rs` |
| `crates/axum-template/src/error/` | `mod.rs`, `code.rs`, `mapping.rs` |
| `crates/axum-template/proto/` | the RPC contract (`template/v1/template.proto`) — `build.rs` generates messages, the service and the descriptor set |
| `crates/axum-template/tests/` | `common/` harness plus one file per contract (`docs/testing.md`) |
| `crates/axum-template/static/` | embedded SPA assets (`rust-embed`; must stay non-empty) |
| `toasty/` | committed migrations and snapshots |
| `docs/` | `architecture.md`, `development.md`, `configuration.md`, `testing.md` |

## Development Commands

```bash
cargo check --all-targets [--features webtransport]   # lib, bins, tests
cargo fmt --all                                       # rustfmt.toml needs nightly
cargo clippy --all-targets --features webtransport    # 0 warnings expected
cargo test [--features webtransport]                  # needs a reachable PostgreSQL
cargo run --bin axum-template                         # server; `-- config|init|role …` for the CLI
cargo run --bin axum-template -- config               # prints the effective config as JSON
```

`./manage` wraps the two binaries (run it from the repository root):

```bash
./manage migrate                              # apply pending migrations
./manage fresh                                # drop all tables, then re-apply (interactive: needs a TTY)
./manage migration-generate --name <name>     # diff models → SQL (interactive when a drop is detected)
./manage init                                 # seed default roles/permissions (idempotent)
./manage create-superuser [-u <name> -p <pw>] # prompts when omitted (needs a TTY)
./manage dev                                  # cargo watch hot-reload (needs cargo-watch)
```

Migration workflow: edit `domain/model/` → `./manage migration-generate --name <x>` → review the SQL
**and** the snapshot → `./manage migrate` → commit both. The template ships one folded base migration
(`toasty/migrations/0000_init.sql`, snapshot `0000_snapshot.toml`); a change that was never released is
folded into it rather than becoming a second file. Two consequences an editor must respect: restore the
hand-added `files_state_attached_at_consistent` CHECK after any regeneration, and refresh a database
created from an earlier `0000_init.sql` with `./manage fresh` (+ `./manage init` and the superuser)
rather than expecting `./manage migrate` to fix it. Why, and the rest of the mechanics:
`docs/development.md`.

## Code Conventions & Common Patterns

- **Formatting**: `cargo fmt` with `rustfmt.toml` (`reorder_imports`, `group_imports =
  "StdExternalCrate"`, `imports_granularity = "Crate"`) — the last two are nightly-only, which is why
  `rust-toolchain.toml` pins nightly.
- **Lints are part of the contract**: workspace `deny` on `unsafe_code`, `dbg_macro`, `todo!` and
  `unimplemented!` (`Cargo.toml`), plus `#![cfg_attr(not(test), warn(clippy::unwrap_used))]` in every
  crate root — production code avoids `unwrap`; use `expect("…")` with a reason where a panic is right.
- **Language**: everything a user, operator or client can observe is English — panic and `expect`
  messages, `tracing` logs, CLI output, the OpenAPI document and the proto contract. A `///` on a
  `#[utoipa::path]` handler or a `ToSchema` item **becomes** published client text (first paragraph →
  `summary`, the rest → `description`), so those `///` are English prose; an internal remark is a `//`.
  `tests/openapi_test.rs` fails on any CJK text in the document or the proto.
- **Comments**: a doc comment states what an item is, or a fact the code cannot express — wire format,
  auth level, invariant. Never restate the name, never explain a change. A property the compiler or a
  constructor already enforces (an exhaustive `match`, a private field behind a validating constructor)
  is not such a fact. Inner `//` comments explaining *why* are welcome.
- **Route module shape**: a `//!` line stating what the module is for, then the handlers top-down, each
  paired with the `#[utoipa::path]` that is the only place its path, params, body, responses and
  accepted credentials (`security(…)`) are written, then `pub const PATH`, then `pub fn router()`
  **last** — listing its routes in handler order,
  ending `.with_tags([…])` and, unless public, `.route_layer(auth(ctx, <backend>))`. `router()` takes
  `&AppContext` only when the module needs state (`health`, `static_site`, `error` take none). Mounting
  lives only in `app/router.rs::create_router` — `nest(PATH, router(&ctx))` for a group,
  `merge(router(&ctx))` for a module that owns its own paths (`app/rpc`, `socket/realtime`,
  `routes/media.rs`). The first tag becomes the group half of every `operationId`, so name it after the
  mount surface (`users`, `files`) or the area (`auth`, `jwt`). One module is one group with one policy;
  a module that must split one mounts sub-routers (`routes/auth.rs`).
- **Extractors**: always the wrappers — `AppPath`, `AppQuery`, `AppJson`, `FormData` (they run
  `validate()`; the raw axum extractors skip it). Identity and permission extractors first, body
  extractor last: the last position is forced, the parts extractors run in declaration order and the
  leftmost failure decides, so a `403` always beats a body or path verdict — `tests/permission_test.rs`
  pins that precedence.
- **Error model**: `AppError` (`#[non_exhaustive]`) has three regions — `Business { code, message,
  errors }` (the only client-actionable failure, must name an `ErrorCode`), five transport variants with
  no code, and internal variants that map to a generic 500. Constructors: `business`, `business_msg`,
  `malformed_request`, `unauthenticated`, `forbidden`, `not_found`, `internal`, `config`, `wrap`, plus
  `bail!(err)`. `From` impls exist only for foreign types (so `?` works) — never add an
  `ErrorCode → AppError` conversion. A new code is one line in `error/code.rs` **plus** its arm in
  `app/http/error.rs::ErrorCode::status()` (exhaustive, no wildcard) in the same commit; both
  `#[serde(rename)]` and `#[schema(rename)]` are mandatory there (utoipa cannot see the macro's
  interpolation). Native-extractor rejections become a codeless `400`, except an over-limit body, which
  keeps `413 file.too_large`.
- **Validation split**: the invariant is declared once in `domain/vo/`; the DTO's
  `#[validate(custom(…))]` only re-shapes that error into a field-level one, and the handler converts
  into the value object (`PaginationReq` → `Pagination` is the pattern). Credentials are the same shape:
  `Username` (3–20 chars) and `Password` (8–32 chars, counted in `char`s) in `domain/vo/credential.rs`
  are the only declaration of the policy, and every setter takes them; `PasswordHash::verify` takes a
  bare `&str`, because the policy does not apply when checking a password.
- **Naming**: `snake_case` modules; per-entity id newtypes generated by `id_type!` (`UserId`, `RoleId`,
  `SessionId`, `RefreshTokenId`, `UserRoleId`, and `FileId` = an unguessable uuid-v7 handle), so a
  swapped argument does not compile; `XService`; input `XReq`, output `XResp`, list `XListResp`; error
  codes `section.snake_case`.
- **Services and async**: a service is a struct holding cloned handles (a `toasty::Db`, the
  `MediaStore`, `Sockets`), plain `async fn`, `Result<T> = Result<T, AppError>`; `#[async_trait]` only
  for `FromMultipart`. No blocking IO in a handler — CPU work goes through `spawn_blocking`.
- **DTOs**: input types derive `Deserialize + Validate + ToSchema`; a query DTO also implements
  `IntoParams` with `#[into_params(parameter_in = Query)]` (a bare `params(Type)` defaults to `Path`)
  and repeats each bound as a literal in `#[param(…)]`, because utoipa rejects a const path there.
  Path arguments are declared on the `#[utoipa::path]` attribute, since every extractor here is a
  wrapper utoipa cannot read through.
- **Downloads**: `app/http/response.rs::Serving` owns the serving contract for both representations
  (type, length, disposition, `nosniff`, cache, validator) — `Serving::of` is the original (raster
  inlined with the RFC 8187 filename, any other type an attachment) and `Serving::thumbnail` a
  derivative (`inline`, no filename, `Vary: Accept`, validator `{sha256}-v1-{size}-{ext}`). A draft is
  `no-store` with no validator, an attached file `immutable` for a year; `not_modified` answers
  `If-None-Match` only when a validator exists, so a handler never asks `is_attached` itself.

## Important Files

| Path | Purpose |
|---|---|
| `crates/axum-template/src/app/router.rs` | the only assembly point (paths, tags, middleware order) |
| `crates/axum-template/src/app/context.rs` | `AppContext`, accessors |
| `crates/axum-template/src/app/startup.rs` | `boot()` / `serve()` / drain-bounded shutdown |
| `crates/axum-template/src/app/http/error.rs` | `ErrorResp`, `ErrorCode::status()`, rejection mapping |
| `crates/axum-template/src/app/http/response.rs` | `Serving`, `Disposition`, `Content-Disposition`, `If-None-Match` |
| `crates/axum-template/src/app/http/openapi.rs` | `with_tags`, `prefix_operation_ids`, `describe_service`, `register_error_schemas` |
| `crates/axum-template/src/app/identity/{mod,backend,session,jwt}.rs` | `Resolved`, `Auth<B>`, the three backends, cookie/JWT resolution |
| `crates/axum-template/src/app/middleware/auth.rs` | the per-group auth layer (hand-written `Service`) |
| `crates/axum-template/src/app/extractor/{permission,multipart}.rs` | `Require<P>`, `FormData` + 413 mapping |
| `crates/axum-template/src/app/routes/media.rs` | the public media surface (original vs `?size=` derivative, `Accept` negotiation) |
| `crates/axum-template/src/app/socket/{protocol,pump,realtime,webtransport}.rs` | frame contract, outbound pump, `/ws` sample, WebTransport sample |
| `crates/axum-template/src/app/rpc/{mod,service,error}.rs` | mount + reflection, the `TemplateService` impl, `AppError` → Connect |
| `crates/axum-template/proto/template/v1/template.proto` | the RPC contract (service, messages, `ErrorDetail`) |
| `crates/axum-template/src/error/{mod,code,mapping}.rs` | `AppError`, the `ErrorCode` catalogue, foreign mappings |
| `crates/axum-template/src/domain/service/*.rs` | use cases; `file.rs` owns row + blob, `thumbnail.rs` is the pure pipeline |
| `crates/axum-template/src/domain/db/mod.rs` | `app_models!`, `init_db`, `ToastyResultExt` |
| `crates/axum-template/src/domain/vo/{permission,credential,id,pagination,file_state,thumb}.rs` | the invariant layer |
| `crates/axum-template/src/infra/{storage/mod.rs,realtime/*.rs,scheduler/mod.rs,signal.rs,logging/mod.rs}` | media store, Hub/Sockets, interval tasks, `draining`, tracing |
| `crates/axum-template/src/config/{schema,mod,paths,env,trusted_proxies}.rs` | config schema, layering + `validate()`, path/env resolution, proxy trust |
| `crates/axum-template/tests/common/server.rs` | `TestServer`, `Session`, `Account`, the request builders |
| `manage` | Bash wrapper over `cargo run --bin …` |

## Runtime/Tooling Preferences

- **Toolchain**: nightly is pinned in `rust-toolchain.toml` (edition 2024 plus the nightly-only rustfmt
  options). No CI, Dockerfile, `clippy.toml`, `deny.toml` or `.cargo/config.toml` — run the checks
  locally.
- **Workspace**: `members = ["crates/*"]`, `resolver = "3"`, versions centralized in
  `[workspace.dependencies]` — a new dependency goes into **both** the workspace table and the crate's
  `{ workspace = true }` list. `Cargo.lock` is gitignored (`.gitignore:28`).
- **Bin targets are implicit**: `src/main.rs` → `axum-template`, `src/bin/toasty-cli.rs` → `toasty-cli`.
- **Feature**: exactly one — `webtransport = ["dep:wtransport"]`, off by default; it gates
  `app/socket/webtransport.rs` and `tests/webtransport_test.rs`, and `validate()` refuses
  `webtransport.enabled` on a build without it.
- **Build needs `protoc`** on `PATH` (`PROTOC` overrides): `build.rs` compiles `proto/` through
  `connectrpc-build` into `OUT_DIR` plus `template.fds.bin`, the descriptor set gRPC reflection
  publishes. The script re-runs when the proto changes, not on every build.
- **Key dependencies**: `tokio` (multi-thread), `axum` 0.8 (`ws`, `macros`, `multipart`), `toasty` 0.11
  (+`toasty-cli` 0.11 as a *runtime* dependency, because the migration CLI parses `Toasty.toml` relative
  to the CWD), `connectrpc`/`buffa` for the RPC surface, `utoipa` 6 (+`-axum`/`-scalar`/`-swagger-ui`),
  `validator`, `jsonwebtoken` 11 (`rust_crypto`), `argon2`, `image` 0.25 with a **hand-picked codec list**
  (`jpeg`, `png`, `webp`, `avif` — the accepted upload types come from it), `jiff`, `clap` 4, `config`
  0.15 (`convert-case`), `rust-embed`, `mime_guess`, `ipnet`, `dotenvy`.
- **Local state**: with `AXUM_TEMPLATE_LOCAL_MODE` or a `.axum-template/` directory present, config,
  logs and media live in `.axum-template/` (gitignored); otherwise under the XDG config/data dirs.
- Configuration keys, environment variables and validation rules: `docs/configuration.md`. Build,
  dependency, protobuf, migration and CLI detail: `docs/development.md`.

## Testing & QA

- Everything that touches the app goes through `crates/axum-template/tests/common/`: `TestServer::start()`
  (or `start_with(|config| …)`) loads the real config, points it at a per-test scratch database and temp
  media dir, seeds the default roles and mounts the real router. `server.login(name, role)` returns a
  `Session`; requests chain `server.post(&uri).cookie(&s).json(&body).send().await?` into a
  `TestResponse` with `expect(status)` / `ok()` / `expect_code("auth.invalid_session")` — those panic
  with the request line, status and body, so prefer them over hand-written assertions.
- Requests run through `oneshot` on the router (no listener); WebSocket and generated-RPC-client tests
  use `server.addr`. Backdating a row (`age_session`, raw `tokio_postgres`) is the way to test
  time-dependent behavior — **no `sleep`** for ages.
- The configured `database.url` must be reachable (override with `AXUM_TEMPLATE__DATABASE__URL`), and
  every `TestServer` issues `CREATE DATABASE`/`DROP DATABASE` on it — **never point a test run at a
  shared or production cluster**.
- `cargo test --features webtransport` is the full suite; a new endpoint is not "done" until it has been
  exercised against the real surface (curl, the SPA, a WS or RPC client) — a test run alone is not proof.
- Add tests only for an uncertain edge or when asked: pin observable contract (status + `code`,
  headers, DB/WS state), never wiring, incidental defaults or message wording. Fix the source, not the
  symptom, and migrate every caller in the same commit — no shims, aliases or deprecated paths.
- Harness API, how to run the suite, and what it deliberately leaves unpinned: `docs/testing.md`.
