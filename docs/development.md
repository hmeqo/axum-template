# Development, build and migrations

Depth for `AGENTS.md` → *Development Commands* and *Runtime/Tooling Preferences*. This file carries the
mechanics; the rules an editor must follow live in `AGENTS.md`.

## Toolchain and build

- **Nightly is pinned** (`rust-toolchain.toml`: `channel = "nightly"`). It is required by
  `rustfmt.toml`'s unstable options, not by any crate feature.
- **Workspace**: `members = ["crates/*"]`, `resolver = "3"` (`Cargo.toml:1-3`); one member,
  `crates/axum-template` (`edition = "2024"`). Dependencies are centralized in
  `[workspace.dependencies]` and referenced as `{ workspace = true }` — adding a dependency means
  editing both `Cargo.toml` files. `Cargo.lock` is gitignored (`.gitignore:28`), so a fresh clone
  resolves versions itself.
- **Lints**: `[workspace.lints.rust] unsafe_code = "deny"` and
  `[workspace.lints.clippy] dbg_macro`/`todo`/`unimplemented = "deny"` (`Cargo.toml:83-89`), adopted by
  the crate through `[lints] workspace = true`. Every crate root adds
  `#![cfg_attr(not(test), warn(clippy::unwrap_used))]` — so `todo!()`/`dbg!()`/`unsafe` are hard
  errors and `unwrap` is a warning outside tests.
- **No CI, Dockerfile, `clippy.toml`, `deny.toml`, `.cargo/config.toml`, Makefile or justfile.** Checks
  run by hand; `manage` is the only script.
- **Bin targets are implicit**: `src/main.rs` → `axum-template`, `src/bin/toasty-cli.rs` → `toasty-cli`.
- **Feature**: `webtransport = ["dep:wtransport"]` is the only one; it gates
  `src/app/socket/webtransport.rs` and `tests/webtransport_test.rs`, and `validate()` refuses a config
  that enables it on a build without the feature.

The checks are listed in `AGENTS.md` → *Development Commands*. `cargo doc --document-private-items`
renders the docs the code carries itself.

## Protobuf surface

- One proto file: `crates/axum-template/proto/template/v1/template.proto` (`package template.v1`,
  `service TemplateService { Me, ListUsers }`). The well-known imports (`google/protobuf/empty.proto`,
  `timestamp.proto`) come from `protoc`'s bundled copies — nothing is vendored, and nothing pins a
  `protoc` version.
- `build.rs` runs `connectrpc-build` over that one file: it emits the generated service module and
  `template.fds.bin`, the descriptor set reflection publishes. It **shells out to `protoc`** (`PROTOC`
  env wins, else the binary on `PATH`) and emits `cargo:rerun-if-changed` for its inputs, so the script
  re-runs when the proto changes — not on every build.
- Integrating a new proto means editing `build.rs` (`files`/`includes`), the mount in `app/rpc/mod.rs`
  and the service impl in `app/rpc/service.rs`.
- Generated code is pulled in once: `pub mod proto { connectrpc::include_generated!(); }`
  (`app/rpc/mod.rs:14-16`). The descriptor set is embedded with
  `include_bytes!(concat!(env!("OUT_DIR"), "/template.fds.bin"))` and served by gRPC server reflection
  at `/api-docs/rpc` (`app/rpc/mod.rs:29,41-46`).
- The descriptor set is emitted **without `SourceCodeInfo`**, so proto comments never reach reflection;
  the `.proto` file's own `//` are the only place a client author reads a field's meaning. Proto3 JSON
  support comes from `connectrpc-codegen`'s defaults (not from this repo's `build.rs`) — replacing
  `connectrpc-build` with a raw buffa/prost pipeline would silently drop it.

## Dependencies worth knowing

| Crate | Why it matters |
|---|---|
| `toasty` 0.10 (`jiff`, `postgresql`, `serde`) | the model layer; models are registered in `app_models!` (`domain/db/mod.rs:11-22`) which both `init_db` and `toasty-cli` consume |
| `toasty-cli` 0.10 | a **runtime** dependency: `Config::load()` reads `Toasty.toml` relative to the process CWD, which is why migrations must be launched from the repository root (that is what `manage` does) |
| `axum` 0.8 (`ws`, `macros`, `multipart`) + `axum-extra` (cookie) | routing, sockets, multipart; the cookie extractor is `CookieJar` |
| `connectrpc` 0.9 (+`-build`, `-reflection`), `buffa`, `buffa-types` | the RPC surface; `-reflection` is `default-features = false` in the workspace |
| `image` 0.25 (`default-features = false`, codecs `jpeg`, `png`, `webp`, `avif`) | **the accepted upload/thumbnail types come from this list** — adding a format means editing it |
| `utoipa` 5 (+`-axum`, `-scalar`, `-swagger-ui`) | `OpenApiRouter` is the routing primitive here, so route modules are `utoipa_axum` routers |
| `config` 0.15 (`toml`, `convert-case`) | the `__` separator plus Kebab conversion behind `AXUM_TEMPLATE__SECTION__KEY` |
| `jsonwebtoken` 11 (`rust_crypto`) | non-default backend; access tokens are HS256 |
| `rust-embed` 8.5 (`axum`) | embeds `static/`; the folder must exist at compile time and stay non-empty. Without the `debug-embed` feature a debug build reads from disk, so SPA edits need no rebuild |
| `tokio` (`full`), `tower`/`tower-http`, `validator`, `jiff`, `sha2`, `ipnet`, `percent-encoding`, `mime_guess`, `argon2`, `clap`, `inquire`, `dotenvy`, `dirs` | the usual support set |

## Database and migrations

- `Toasty.toml`: `path = "toasty"`, `prefix_style = "Sequential"`, `checksums = false`,
  `statement_breakpoints = true` — the library defaults, restated. The DB URL comes from this
  application's own config, so `AXUM_TEMPLATE__DATABASE__URL` (or `config.toml`) decides where
  migrations run.
- Committed state: one migration `toasty/migrations/0000_init.sql` (tables `users`, `roles`,
  `user_roles`, `sessions`, `refresh_tokens`, `files`), its snapshot `toasty/snapshots/0000_snapshot.toml`
  and the registry `toasty/history.toml` (id + snapshot name). Column order is snapshot-significant:
  regenerate the SQL and the snapshot together, never edit one alone.
- The base migration carries one **hand-added** constraint the generator does not know about
  (`files_state_attached_at_consistent`, `0000_init.sql:29-30`). It has no representation in the
  snapshot, so tooling keeps reporting "no changes" while it is missing from a database — restore it
  after any regeneration.
- **Folding**: a schema change that was never released is folded into `0000_init.sql` (and its snapshot)
  instead of becoming a second file. Consequence: a database created from an earlier `0000_init.sql`
  never receives the folded change — `migration apply` reports nothing pending (the history records the
  file as applied by id) and the next query that materializes the full row fails with
  `column … does not exist`. Refresh with `./manage fresh` + `./manage init` + `create-superuser`, or
  add the columns by hand to keep the data. A *released* change must be a new file that sorts after
  `0000_init.sql` — the test harness replays `toasty/migrations/*.sql` in filename order
  (`tests/common/scratch.rs`).
- New models: edit `domain/model/`, register in `app_models!`, regenerate, then `./manage migrate`.
- `manage` is a Bash script that `cd`s to the repository root first (so `Toasty.toml` resolves) and
  dispatches one subcommand per function — the script is the list of them; an unknown subcommand is a
  shell error.
- **TTY constraints**: `./manage fresh` always asks for confirmation (`dialoguer::Confirm`, not skipped
  by `--skip-migrations`) and `create-superuser` prompts when `-u`/`-p` are omitted — both need a
  terminal. In a non-interactive environment use `./manage migrate` plus explicit SQL instead.
- The underlying `toasty-cli` binary also exposes `migration {apply,generate,snapshot,drop,reset}`;
  `manage` does not wrap `snapshot` or `drop`, and `drop` deletes the SQL, the snapshot and the history
  entry (it does not undo the database).

## CLI (`axum-template`, clap)

No subcommand starts the server (`app::serve()`). **Any** subcommand goes through
`app::startup::boot()` first — loads `.env`, loads and validates the config, initializes tracing,
connects to PostgreSQL and ensures the media directory. So `config` and `perms` also need a reachable
database, and every subcommand has that side effect.

`axum-template --help` (and `… role create --help`) is the flag reference: each subcommand and argument
carries its clap help from `cli/command.rs`, so the list is not repeated here. What clap cannot say:

- `config` prints the whole effective `ConfigData` as JSON — `auth.jwt.secret` in clear text included.
- `init` is an idempotent RBAC seed (existing roles are skipped) and prints a created/skipped report.
- `role create` takes comma-separated permission codes; an unknown code is a `malformed_request`, and
  `role delete` a missing name a `not_found`.
- `perms` prints every `Perm` as `code - description`; it reads no table, but booting still needs the
  database.
- There is no `--version` flag (the clap command has no `version` attribute).

## Local state, ignore rules

- The config/data directory is resolved once, before anything else runs; `docs/configuration.md` owns
  the branches and the caveats. What matters here is that logs and media follow that same directory.
- `.gitignore` covers `Cargo.lock`, `target/`, `debug/`, `**/*.rs.bk`, `.env`, `config/local.toml`,
  `*.db`, `logs/` and `.axum-template`.
- Known residue, do not mistake for current behavior: the repository-root `logs/` directory (the code
  writes to `<data-dir>/logs/access.log`, rotated daily) and the `.gitignore` entry `config/local.toml`
  (there is no `config/` directory and no loader reads that file).
