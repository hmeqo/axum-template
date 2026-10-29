# Axum Template

A Rust reference API service you copy from: Axum 0.8 and Toasty 0.11 on PostgreSQL, nightly
toolchain. It is a template, not a framework — take the patterns the target service needs and delete
the rest.

## What you get

- **Auth** — a session cookie per device (sliding expiry, per-device revocation) and JWT
  access/refresh pairs with single-use rotation; the route group picks the credentials it accepts.
- **RBAC** — one `Perm` catalogue, roles seeded by `./manage init`, a CLI for roles and permissions.
- **Users** — paginated, filterable listing; self-service rename and password change; an admin
  reset that revokes the target's sessions.
- **Files** — multipart upload as a draft, attach, then a public media route serving the original or
  an `<img>`-sized AVIF/JPEG derivative on demand.
- **Realtime** — `/ws` over a per-topic hub, with session revocation closing the socket; an optional
  WebTransport sample behind a cargo feature.
- **RPC** — one `.proto` generates the messages, the service trait and the descriptor set, so the
  ConnectRPC surface and the gRPC reflection schema cannot drift from the code.
- **HTTP edge** — health probes, request ids and timeouts, CORS, trusted proxies, one error body
  (`{code?, message, errors?}`) over a closed code catalogue, layered config, embedded SPA.

## Quick start

```bash
# Needs a reachable PostgreSQL and `protoc` on PATH.
./manage migrate          # create the schema
./manage init             # seed the default roles and permissions
./manage create-superuser # prompts for credentials

cargo run --bin axum-template   # or `./manage dev` for hot reload (needs cargo-watch)
```

Point `database.url` at your PostgreSQL first; `docs/configuration.md` covers the rest.

## Exploring the API

`/api-docs/scalar` is the interactive reference — log in with the superuser you created and call the
endpoints from there; `/api-docs/swagger-ui` shows the same document. `/api-docs/openapi.json` is
the document itself, generated from the handlers, so it is the endpoint list rather than a second
copy of one. RPC clients read the schema the server was compiled from, through gRPC reflection at
`/api-docs/rpc/grpc.reflection.v1.ServerReflection/ServerReflectionInfo`.

## Where to read more

| File | Carries |
|---|---|
| `docs/architecture.md` | why the code is shaped this way: layering, request path, auth, realtime, storage, OpenAPI |
| `docs/configuration.md` | configuration precedence, environment variables, where files live |
| `docs/development.md` | toolchain, protobuf, migrations, the CLI |
| `docs/testing.md` | the test harness, how to run the suite, what it does not pin |
| `AGENTS.md` | the rules this repository is written by, and the file indexes |

## License

MIT — see `LICENSE`.
