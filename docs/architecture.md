# Axum Template Architecture

The executable rules live in `AGENTS.md`; this document is the long-form companion — why the code is
shaped that way, in the code's own terms. Build, tooling and migrations are in `docs/development.md`,
configuration and environment in `docs/configuration.md`, the test harness in `docs/testing.md`. A fact
has one home in one of these files, and code is the source of truth: a line here that contradicts it is a
bug.

## Layering

```
app/        HTTP edge and assembly. Root files wire only (context: dependency assembly,
            startup: boot and shutdown, router: routes and document assembly); http/ is the shared
            representation layer (error, response, openapi) used by every endpoint;
            routes/ dto/ extractor/ middleware/ are the endpoint surface; identity/ is shared
            authentication; socket/ holds long-lived connections.
domain/     Business implementation: model (Toasty), service (use cases), vo (value objects),
            db (connection and error mapping).
infra/      logging (tracing), scheduler (interval tasks), storage (media on disk).
config/     schema, layered loading and validation, path and environment resolution.
cli/        command-line adapter (clap).
```

`domain/` holds no loose files. `app/`'s root files follow the same rule: `router.rs` contains no
route-specific handler body (the `/api` fallback lives in `routes::api_not_found`), only the merge
and nest of the module routers and the mounts of the doc UIs and `/ws`; the document post-processing
helpers it invokes live in `http/openapi.rs` and `app/rpc`.

## Data flow

```
HTTP → routing → middleware (trace → cors → timeout → body-limit) → group auth layer → extractors
  → Handler (protocol conversion only) → Service (business) → Toasty → PostgreSQL
```

Handlers reach services through `ctx.srv()`, configuration through `ctx.cfg()`, media storage through
`ctx.media()`. `AppContext`'s fields are private; only the accessors are public.

A `/api` response that declares no caching of its own gets `Cache-Control: no-store` (the same layer
that scopes `request_id`): every one of them is per-user, and a request that slid its session also
carries `Set-Cookie`. Static assets and the media download route are outside that subtree and keep
their own policies.

## Intentional deviations from classic DDD

Against Vernon's `iddd_identityaccess` (*Implementing Domain-Driven Design*):

1. No per-aggregate directories; horizontal layering (`model/`, `service/`).
2. No repository interfaces — services call Toasty directly
   (*Persistence and layering* below).
3. No separate application layer — use cases live on the concept's service
   (`AuthService::login_session`).
4. Value objects are collected in `vo/` rather than distributed per aggregate.
5. `vo/` depends on `serde` / `validator` / `utoipa` / `toasty`: ids such as `UserId` and `Perm` are
   referenced directly by DTOs. `id.rs` uses serde + `validator::ValidateRange` + `ToSchema` +
   `toasty::Embed`, `permission.rs` serde + strum + `ToSchema`, `file_state.rs` serde + `ToSchema` +
   strum + `toasty::Embed`, and `password_hash.rs` serde + `ToSchema` + `toasty::Embed`;
   `credential.rs`, `pagination.rs` and `page.rs` use none of them. Request
   and response **shapes** still live only in `app/dto/` — which is why `PaginationReq` is in
   `app/dto/pagination.rs`, not `vo/`; the domain-side half of a listing is `Page<T>`
   (`vo/page.rs`), which the DTO's `From` impl turns into that shape.

## Persistence and layering

Toasty is the persistence port and it stops at `domain/`: `app/` never names a Toasty model or query
type (the `Db` handle is passed through for assembly only), and the migration CLI imports the models
only to generate migrations. An ORM already owns connections, query
building, result mapping, `include`-style eager loading, pagination and transactions, so a
`Repository<T>` on top would either forward calls unchanged or re-expose the query DSL while hiding
exactly those features. Two shapes earn their keep instead:

- **Named query shapes** — a query lives in the service that owns its aggregate; a read shape the
  owning service names rather than writes inline (a paginated list with its count, a delete-by-owner
  pair) is a free function in `domain/db/`. Relations are read through the model's association paths
  (`#[has_many]` / `#[belongs_to]` + `include`), not as a hand-rolled two-step load.
- **Deletes cascade through the declarations** — a child whose foreign key is required is deleted
  together with its parent, an optional one has its key set to `NULL`. Toasty plans those statements
  application-side (the schema carries no `ON DELETE` clauses) and wraps a multi-statement plan in its
  own transaction, so a service calls the generated `delete_by_id` for the parent and keeps no delete
  graph of its own.

Ports are for infrastructure that can be swapped, which is why `MediaStore` is one (local directory
now, object storage later) and PostgreSQL is not. Nor does testing argue for a repository: the
`TestServer` harness needs a reachable PostgreSQL and creates a scratch database per test, dropping it
on teardown, so a mockable data-access trait would buy nothing.

## Lifecycle

`app/startup.rs::boot()` is the only boot path: load the optional `.env` from the config directory →
load and validate the configuration → initialize tracing → build `AppContext` (connect the database,
build services, ensure the media directory). `serve()` adds `create_listener()` and enters
`serve_with_drain`; CLI subcommands go through the same `boot()`.

Shutdown: `trigger_shutdown()` notifies long-lived connections (a WebSocket receives close code 1001)
→ axum's graceful shutdown starts → the drain is bounded by `server.shutdown-timeout-seconds` → the
log `WorkerGuard` is dropped to flush buffered lines.

Long-lived connections wait with `infra::signal::draining(&mut rx)`, **not** `rx.changed()`: a receiver
subscribed after `trigger_shutdown()` never observes a change, so the connection would only be cut
when the drain timeout expires. `draining` wraps `wait_for`, which returns immediately when the value
already satisfies the predicate, and keeps the non-`Send` `Ref` inside the function. It is generic
over the signal: the same call waits on the context's shutdown flag, on a `Sockets` revocation
signal and on a pump's own stop flag.

`AppContext` wraps an `Arc<AppContextInner>` and derefs to it, so axum's per-request `State` clone is
a refcount bump rather than a copy of `Services` (which holds the JWT key) and of `MediaStore`'s paths.

Probes: `/health` (liveness) and `/health/ready` (database reachable). Both are operational surface
and are not part of the OpenAPI document.

## Authentication and authorization

Three jobs, three places:

- **Backend** (`app/identity/backend.rs`): `AuthBackend` is one backend per route group, and each group
  names its own at the mount, `.route_layer(auth(ctx, …))`, so the policy is readable next to the routes
  it governs: `SessionAuth` (cookie only), `JwtAuth` (`Authorization: Bearer` only) and `JwtOrSession`,
  which composes the two — a backend is the unit of composition, so another credential source is another
  implementation.
  `routes/auth.rs` splits in two: the session group (`login` is public; `logout`, `sessions`,
  `revoke` take the cookie only) and the self group (`GET /me`, `PUT /password`), which takes either
  credential because the credential is its subject.
  `routes/jwt.rs` takes the `Bearer` token only, and `routes/user.rs`, `routes/files.rs` and
  `app/rpc` take `JwtOrSession`.
- **Identity** (`app/identity/mod.rs`): `JwtOrSession` walks its sources, and the first credential that is
  *present* decides. Present but invalid is a 401 and the walk stops there, so an `Authorization` header
  is never quietly downgraded to a valid cookie. The verdict lands in the request extensions as the one
  `Resolved` value (`Authenticated` / `Invalid` / `Absent`), and that resolution is the only place a
  credential is parsed.
- **Authorization** (`app/extractor/permission.rs`): `Require<P>` reads the cached identity and checks the
  `Perm`, never a credential. Both read the cache, so a group whose layer is missing
  fails closed — 401, not a silent downgrade — and a 401 always precedes any 403.
- A 401 from a group that accepts `Bearer` carries `WWW-Authenticate: Bearer realm="api"`, plus
  `error="invalid_token"` when a token was presented and rejected; cookie-only entries send no challenge,
  since a cookie has no registered scheme (RFC 9110 §15.5.2 wants one). `/ws` mounts
  `SessionAuth` like every other group and takes `Auth<SessionAuth>` in its upgrade
  handler: the sample is session-only, but nothing about it bypasses the layer.

The extractor names the group's backend — `Auth<SessionAuth>`, `Auth<JwtAuth>`, `Auth<JwtOrSession>`,
the same shape as `axum-login`'s `AuthSession<Backend>` — and destructures to the identity that backend
yields (`SessionIdentity` for the session backend, the subject `CurrentUser` for the other two), while
which credentials a group accepts stays the backend's business: the backend both resolves the request
and builds the payload (`AuthBackend::identity`, plus `optional_identity` for the optional form).
`Option<Auth<SessionAuth>>` adds "the session when there is one" (`None` for a bearer token).
`session::resolve`
(`app/identity/session.rs`) reads the configured cookie (default name `session_id`) and looks up the
`sessions` table by `token`; its writes are deleting an expired row and pushing a session that is past
half its TTL to `now + ttl`. `jwt::resolve`
(`app/identity/jwt.rs`) decodes
`Authorization: Bearer <token>` and never touches the database. Both hand their result back to their
backend, which hands the layer one `Resolved` — and the session requirement is enforced in one place,
`SessionIdentity::of` (an identity without a session asking for it is a 401).

A session has one expiry, `expires_at`, and it moves on the request path: a session-cookie request with
less than half the TTL left pushes it to `now + ttl`, and the group auth layer re-issues the cookie on
that same response with a matching `Max-Age`, so an active client slides without asking; more than
half a TTL left writes nothing. There is no absolute cap — the mitigations are that logout deletes
the caller's own session, that a password change or reset ends the account's sessions, and that
sweeping expired rows is still open.

The policy lives on `Session` (`domain/model/session.rs`: `is_expired` / `should_extend` / `expiry`),
and each takes `now`, which makes it unit-testable without a database. Services only read and write
rows.

Authorization is a handler argument. A protected route declares `_perm: Require<UserRead>`; the extractor
reads the identity the group layer resolved and checks the `Perm`, so 401/403 short-circuits before the
body is read. No `Require<…>` means no permission requirement; an identity extractor alone means
"authenticated only" — self-service is kept out of the perm-gated group (`PUT /api/auth/password` asks
for identity and the old password, and takes no id because the credential is the subject), while an admin
resetting someone else's password is `Require<UserPasswordReset>` with no old password — and it revokes
the target's sessions and refresh tokens, because a session outlives the password it was created with
(`POST /api/users/{id}/password/reset`); the self-service change revokes every session and all refresh
tokens, then hands the calling device a fresh session in the same response (`PUT /api/auth/password`
plus `Set-Cookie`), so an old cookie dies at the moment the password changes while the caller stays
signed in. The `Perm`
variant, its wire code, its description and the marker type are all generated by `perms!` in
`domain/vo/permission.rs`, the catalogue's single declaration site.

Login belongs to `AuthService`; session logout and revoke go through `SessionService` (the one place that
keeps rows and live connections in step); permission aggregation has one
implementation (`RoleService::get_user_permissions`). Logout deletes the caller's own session and clears
the client cookie (JWT: every refresh token of the user).

The credential policy has one declaration site too: `Username` (3–20 characters) and `Password`
(8–32 characters, counted in `char`s) in `domain/vo/credential.rs`. Every setter requires those types
— `UserService::create` / `update_username` / `change_password` / `reset_password`,
`bootstrap::create_superuser`, `PasswordHash::hash` — so no route or CLI path can bypass the policy.
The DTO's `#[validate(custom(…))]` only re-shapes the value object's error into a field-level one.
`PasswordHash::verify` takes a bare `&str`: the policy is not applied when checking an existing
password.

## Realtime and RPC

- **The substrate** (`infra/realtime/`): `Hub<E>` fans events out per `Topic` — one broadcast channel
  per topic, opened with the first subscriber and reaped with the last one, so a topic nobody listens
  to costs nothing; `publish` on a topic with no subscriber returns 0 without creating anything, and a
  subscriber that fell behind is told how many events it missed (`Recv::Lagged`) instead of silently
  skipping them. `Sockets` is the live-connection registry, grouped by user: it owns lifecycle only
  (`register` hands back a revocation signal that `revoke_user` / `revoke_user_except` /
  `revoke_session` fire), never
  delivery. Both are process-local — multi-instance fan-out needs an external bus and is not this
  layer.
- **The frame contract** (`app/socket/protocol.rs`, `app/socket/pump.rs`): reserved control type names
  (`subscribe`/`unsubscribe`/`ping`/`pong`/`resync`/`error`, so no business frame may reuse them), a
  strictly compared control-frame shape (serde's `deny_unknown_fields` does not apply to internally
  tagged enums, so the field set is compared here), and one downlink envelope where a business event
  carries `topic` next to its own fields while an error frame carries the very `{code?, message}` body
  REST writes. A pump task forwards one `Subscription` into a connection's outbound queue, so the
  connection loop is the only writer of its socket: no `split()`, no second writer.
- **`/ws`** (`app/socket/realtime.rs`): the sample, not the framework — it defines `LobbyEvent` /
  `LobbyCommand`, one topic (`lobby`) and the `subscribe` ACL, and `AppEvent` (`app/event.rs`) is the
  single bus every feature adds a variant to. A connection holds a session identity
  (`Auth<SessionAuth>`, the session id is what makes per-session revocation exact) registered in
  `Sockets`; logging out, revoking that session, or a password change (which revokes every session of
  the account, the caller's included) closes it with `CLOSE_REVOKED` (1000, "session revoked" — 1001
  would invite an immediate reconnect), a server drain with `CLOSE_DRAINING` (1001). Rooms, history
  and multi-instance support remain business concerns.
- **ConnectRPC** (`app/rpc/`): the contract is `proto/template/v1/template.proto`; `build.rs` generates
  the messages, the service trait and the descriptor set, `mod.rs` mounts that service under
  `/api/rpc` (guarded) and gRPC server reflection under `/api-docs/rpc` (public, next to the REST
  document — the schema is not a secret, the calls are), and `service.rs` implements the two methods. Because the proto is the
  source of everything there is no frame grammar to hand-write and no document to keep in sync: the
  schema clients read off reflection is the proto the server was compiled from, so an unused message
  or a renamed method shows up at build time rather than as a documentation diff.
  The methods keep the REST split — identity read from the `Resolved` the group layer left in the
  request extensions, one permission check where the REST side writes `Require<P>`, then one service
  call — and `error.rs` maps `AppError` onto Connect's error body: the canonical code comes from the
  same `ErrorCode::status()` table the REST surface uses, and the business code plus field errors ride
  in a `template.v1.ErrorDetail` detail, so both surfaces report a business failure the same way.
  The service is an axum `fallback_service` with no `Route`s of its own, so its group layer is applied
  with `.layer`, not `.route_layer` (which would be a no-op). Reflection is a bidirectional stream, so
  it needs HTTP/2 (h2c is enough) and a unary call to its path is `unimplemented`; it is mounted apart
  from the call surface precisely because it is a handler from another crate with no extractor of ours
  in it, so the group layer could resolve an identity for it and nothing would demand one.
- **WebTransport** (`app/socket/webtransport.rs`, feature `webtransport`): a collaboration room —
  peer state travels as datagrams (high-frequency, droppable), join/leave events each open a new
  unidirectional stream (reliable, ordered, no head-of-line blocking between events), and the roster is
  the first such stream a joining peer gets. It is not an RPC transport, so it has no router and no
  group layer: the CONNECT headers go through the same `JwtOrSession` backend the HTTP groups use
  (`identity::backend::resolve_headers`), and a rejected session is answered `403`, because wtransport
  offers no `401` and no place for a challenge. Fan-out rides the same `AppEvent` bus as `/ws` (its own
  `collaboration` topic), where `Recv::Lagged` means "stale state was dropped"; the peer table only
  feeds the roster. The connection registers in `Sockets` with the session its CONNECT headers named, so
  revoking that session closes it (`CLOSE_REVOKED`). A browser-initiated CONNECT carries no cookies and
  no HTTP authentication, so the sample is usable only by clients that set the headers themselves.

## File storage

A `File` row is the single source of metadata (owner, original name, MIME, size, display width and
height, sha256, lifecycle state), and its primary key is the file's only identity: an unguessable uuid v7
(`FileId`) that is also the public handle and the on-disk directory name. A bigint key would have forced
a second opaque column for the handle, because the media route is public and ids must not be enumerable;
uuid v7 is time-ordered, so it keeps index locality. Upload writes the blob first and the row second (a
failed insert leaves an orphan directory for the GC); deleting a user removes the file row through the
relation cascade and leaves the directory to the GC.

One file is one directory: `{media-dir}/{uuid}/origin` is the blob as uploaded, and
`{media-dir}/{uuid}/thumbs/` holds its derivatives — `v{pipeline}-{side}.{ext}`. That layout is what ties
a derivative's lifetime to its row's: deletion is one `remove_dir_all`, and the reconciler only ever
decides about directories, never about loose files that might belong to somebody else. `width` and
`height` are the display dimensions (EXIF orientation applied) read from the image header at upload, so a
consumer can reserve layout space without decoding anything; they stay `NULL` for non-raster files.

`FileState` is a payload enum (`Draft | Attached { at }`) whose invariant — a draft has no
`attached_at`, an attached file has one — is enforced at the single conversion point between the two
columns and by a database `CHECK`. The wire form is the flat projection (`state` + `attached_at`)
because a client reads fields, not a Rust enum.

The media route serves two representations through one builder (`app/http/response.rs::Serving`):

| Request | Representation | Disposition | Cache |
|---|---|---|---|
| `{media-url-prefix}/{id}` | the original blob | `inline` with the RFC 8187 filename when the type is raster, otherwise `attachment` | draft `no-store`, attached `immutable` |
| `{media-url-prefix}/{id}?size=N` | the `≤N` derivative (AVIF, JPEG fallback) | `inline` | same rule |

A draft is `no-store` with no validator, because the uploader has not committed to it yet; an attached
file is immutable for a year, carries an `ETag` and answers `If-None-Match` with `304`. The original's
validator is the row's sha256; a derivative's appends the pipeline version, the tier and the format
(`{sha256}-v1-256-avif`), so re-encoding can never be served out of a stale cache.

`?size=N` asks for a **box**: the longest side becomes `min(N, longest side of the source)`, the aspect
ratio is the source's, nothing is cropped and nothing is enlarged; `N` must be one of
`[64, 128, 256, 512, 2048]`. The format comes from the request's `Accept` — server preference AVIF, then JPEG,
with `*/*` counting as accepting either — so the response carries `Vary: Accept`; a client that accepts
neither gets JPEG rather than `406`, because the caller is an `<img>`. A non-raster file, or a `size`
outside the list, is a codeless `400`. The bytes are re-encoded however small the source is: a 300×203
PNG can weigh 164 KB, and this endpoint exists to cut bytes, not pixels. Rendering happens in
`spawn_blocking` under a process-wide semaphore (`domain/service/file.rs`), `image::Limits` bounds what a
hostile upload can allocate, EXIF orientation is applied before scaling, and the JPEG tier composites
transparency onto white. A derivative's `Content-Length` is the derivative file's length, never the
row's.

`inline` is safe for a derivative precisely because the bytes are ours: the whitelist is raster only (no
SVG), the declared type is the one we encoded, and `X-Content-Type-Options: nosniff` stays on. A raster
original is inlined for the same reason — `nosniff` plus a declared raster type means the browser only
decodes it as an image, so a new tab shows the picture instead of downloading it — while a non-raster
original keeps `attachment`, which is the stored-XSS class (inline rendering of an arbitrary upload from
the app's own origin) this avoids.

Two maintenance passes keep row and disk consistent, driven by the interval scheduler
(`infra/scheduler/mod.rs`, `files.gc`, `storage.gc-interval-minutes`):

1. **Draft sweep** — rows still `Draft` after `storage.unattached-ttl-hours` are removed together with
   their directories. This is what reclaims an upload the user replaced before saving.
2. **Reconcile** — three checks per directory. A directory without a row older than
   `storage.orphan-grace-minutes` is deleted. A live directory whose `origin` is missing, or whose length
   disagrees with the row, is logged and left alone (business rows are never silently deleted; sizes are
   compared, content is never re-hashed). And a derivative inside a live directory that is not part of the
   current `(pipeline version, size, format)` set — a leftover temp file from a crash included — is
   deleted, because every derivative is regenerable. Changing the encoder, the tier list or
   `THUMB_PIPELINE` therefore needs no backfill: new files appear on demand, old ones go on the next pass.

The scheduler assumes a single instance; running two would need a lease. Tasks stop through the same
`draining` signal as the long-lived sockets, and `serve()` aborts them after the drain.

A conditional request on the *original* is answered before the blob is opened, so a row whose blob is
missing still answers `304` to a matching `If-None-Match` — the plain GET surfaces the broken invariant as
`500`. A derivative request has to materialize (read or render) the representation first, so the same
broken row answers `500` there even with a matching validator.

## Request correlation id

`middleware/request_id.rs` generates a UUID v4 per request and uses the same value in three places:
the request extension, the tracing span, and the `x-request-id` response header.

It is attached only where an API contract exists (the `/api` subtree and the media download route);
static assets, probes and the documentation UIs carry no id. The error-log middleware is declared
outside the `/api` router, so its code after `next.run()` runs after the inner span has exited and
cannot read the id from the span context — it takes the id from the response extensions instead.

## Client address and tracking

The peer address comes from axum's `ConnectInfo`, which only exists when the router is served with
`into_make_service_with_connect_info::<SocketAddr>()` (`app/startup.rs`); the test harness injects the
same extension itself because its requests go through `oneshot` rather than a listener.

`X-Forwarded-For` is read only when the peer matches `server.trusted-proxies` (`config/trusted_proxies.rs`,
the single place the rule and the right-to-left resolution live): an empty list ignores the header
entirely, a trusted peer makes the walk skip trusted hops from the right and take the first untrusted
value, and an all-trusted chain keeps the leftmost one. Addresses are compared canonically, so
`::ffff:10.0.0.5` matches `10.0.0.0/8`.

`app/extractor/client_info.rs` maps that into `domain/vo::ClientInfo` (peer or resolved address plus
`User-Agent`, both optional). The `TraceLayer` span logs `client_ip` only — a `User-Agent` would bloat
every access-log line — while the session rows keep both. A session's only time state is `expires_at`
(pushed forward by the request-path slide, which already writes); the session list publishes it rather
than a "last seen", because without a per-request touch the only thing a store can know is when the
session dies. `refresh_tokens` stores `ip` and
`user_agent` only, because rotation recreates the row and `created_at` is then its last-seen time.

`GET /api/auth/sessions` lists the caller's own sessions (bounded by `MAX_LISTED_SESSIONS`, newest
first, current one flagged) and `DELETE /api/auth/sessions/{id}` revokes one of them, answering 404 for
anything not owned by the caller and clearing the cookie when the revoked session is the caller's own.
The revoke is a two-step read-then-delete because toasty's `Delete<T>` is fixed to `T = ()` and its
`exec()` reports no affected-row count.

## Key types

- Entity ids (`domain/vo/id.rs`): `UserId` / `RoleId` / `SessionId` / `RefreshTokenId` / `UserRoleId`,
  each a transparent wrapper over `i64`, so ids of different entities are not interchangeable.
- `Pagination` (`domain/vo/pagination.rs`): validates `page >= 1`, `MAX_PER_PAGE = 100` and a
  `MAX_OFFSET = 10_000` result window on construction, and owns `offset()` / `limit()`. Offset
  pagination keeps `total` and page jumps; a feed-shaped endpoint would want a separate keyset type
  instead of widening this one.
- `Page<T>` (`domain/vo/page.rs`): what a "list + count" use case returns — items, total and the
  `Pagination` that produced them, so a page and its footer cannot disagree. The service owns the
  pairing (`UserService::page`, `FileService::page`); the wire shape stays the DTO's, built from the
  `Page` in one `From` impl.
- `PasswordHash` (`domain/vo/password_hash.rs`): only `hash()` produces one, and it takes a
  `Password`, so plaintext cannot be stored.
- `Username` / `Password` (`domain/vo/credential.rs`): the only construction constraints on
  credentials; the DTO attributes mirror them for field-level errors.
- `PaginationReq` (`app/dto/pagination.rs`): the query-string shape, converted to `Pagination` before
  the business layer.
- `File` / `FileId` / `FileState` (`domain/model/file.rs`, `domain/vo/id.rs`,
  `domain/vo/file_state.rs`): the stored file, its unguessable uuid handle (which is the primary key,
  so a raw `Uuid` from anywhere else cannot be passed where a file handle is expected) and its
  lifecycle; `FileService` (`domain/service/file.rs`) owns both the row and the blob and holds the
  `MediaStore`.
- `ToastyResultExt::optional()`: maps only record-not-found to `None`. `get_by_id` turns a missing row
  into 404 on top of it; invariant lookups keep `?` and end up as 500.
- `MediaStore` (`infra/storage/mod.rs`): blob IO under `{media-dir}/{id}`, where `id` is the row's
  `FileId`. `delete_dir` deletes only a direct child of the media directory (path components are
  checked), which is what lets the reconciler delete disk entries without trusting their names; the
  name carries no metadata, so a stored blob cannot describe itself.

## Errors

The dependency points one way: the domain produces only `AppError`, and only the HTTP layer knows
that status codes exist.

| Layer | Location | Responsibility |
|---|---|---|
| Carrier | `error/mod.rs` | `AppError` — one enum with business / transport / internal regions |
| Code table | `error/code.rs` | `ErrorCode` — the closed set clients branch on, dot-lowercase names |
| Mapping | `error/mapping.rs` | External error types → variants |
| Representation | `app/http/error.rs` | `{code?, message, errors?}`, `ErrorCode::status()`, rejections |

- `AppError::Business` is the only client-visible business failure and must name a code. A code-less
  failure is either one HTTP semantics already define (`MalformedRequest`, `Unauthenticated`,
  `Forbidden`, `NotFound`, `Unavailable`) or an internal fault, which is always a 500 with generic
  text.
- Status is decided per code: `ErrorCode::status()` is a table in `app/http/error.rs` with no wildcard
  arm, which keeps `error/` free of axum while forcing a status decision for every new code.
  `ErrorCode` derives `EnumIter`, so contract tests iterate all codes.
- A code's granularity is what makes the client behave differently: `auth.invalid_credentials` and
  `auth.session_expired` are both 401 yet separate codes; a malformed JSON body is the same to every
  client and gets no code.
- Database errors: connection faults (`is_connection_lost` / `is_connection_pool`) become
  `Unavailable` (503), everything else stays `Database`. Record-not-found is not classified
  globally — the call site chooses between 404 (id from a URL) and 500 (broken invariant).
- Wire body `{code?, message, errors?}`: `code` is exported to OpenAPI as an enum, `message` is
  human-readable and generic for internal faults, `errors` appears only for field-level validation
  (`validator::ValidationErrors` becomes "field → problems" in `mapping.rs`). Clients check `code`
  before the status.
- The published documents do not enumerate statuses per endpoint: every operation declares
  `ErrorResponses` (`app/http/error.rs`), an `IntoResponses` pair covering `4XX` and `5XX` with
  `ErrorResp`. Which 4xx an endpoint can produce follows from its extractors and the code table, and a
  hand-written list drifted from that (a malformed body or path id was 400 while some operations
  claimed otherwise). Two consequences of the shared type: utoipa does not collect components for an
  `IntoResponses` entry, so `register_error_schemas` at the assembly point registers `ErrorResp` and
  `ErrorCode`, and `tests/openapi_test.rs` fails on any `$ref` without a target.
- Only 5xx are logged; `IntoResponse` attaches an `Arc<AppError>` to the response extensions for
  `middleware::error_log`.

## Routing and OpenAPI

- The API is mounted under `/api` with its own fallback, so unmatched `/api/*` paths return a JSON 404
  (`routes::api_not_found`). The root fallback serves the embedded SPA: real asset, then directory
  index, then `index.html`.
- The nested route modules (`auth`/`jwt`/`user`/`files`/`error`) carry `pub const PATH` plus `pub fn
  router()` and are nested under `/api`; `health` and `static_site` are merged and spell their full
  paths. A nested module's `#[utoipa::path]` writes only its own segment, so the declared path and
  the mounted path cannot drift.
- `operationId` is never handwritten: `app/http/openapi.rs::prefix_operation_ids` adds the group
  prefix at the single document-assembly point, using the operation's first tag (`login` →
  `auth__login`); consumers strip at the first `__`. `tests/openapi_test.rs` fails when an operation
  is left untagged and therefore unprefixed. `describe_service` (same module) replaces `info` with the
  crate's name and version, so the document describes the service that owns it.
- Which credentials an operation accepts is written once, as `security(…)` on its `#[utoipa::path]`,
  and reaches clients because `register_security_schemes` registers the two schemes it can name at the
  same assembly point (`SECURITY_SESSION_COOKIE`, whose cookie name comes from the config, and
  `SECURITY_BEARER`). Two security requirements on one operation mean *either* credential, one means
  that one, and none means public. The entry and the extractor are two readings of the same policy, so
  `tests/openapi_test.rs` checks both ends: a non-public operation must declare a credential, and every
  name must resolve to a component.
- Text in the published documents comes from the source comments, so those comments are written for a
  client: a `///` on a `#[utoipa::path]` handler becomes the operation `summary`/`description`, and a
  `///` on a `ToSchema` / `IntoParams` field becomes that property's or parameter's description. An
  internal remark is a `//`, and a contract test fails when any CJK text reaches either document.
- `axum_extras` resolves nothing from a handler signature here: it recognizes extractors by the type
  names `Path` / `Query` / `Json` / `Form` / `Bytes`, and every extractor in this crate is a wrapper
  (`AppPath`, `AppQuery`, `AppJson`, `FormData`) whose validation is the reason it exists. So
  `request_body = …`, the path arguments and the query parameters are declared on the
  `#[utoipa::path]` attribute. A query DTO is named in `params(…)` instead of being spelled out:
  it implements `IntoParams` with `#[into_params(parameter_in = Query)]` (utoipa defaults a bare
  `params(Type)` to `Path`) and carries the bounds, which must be literals — utoipa rejects a const
  path in both `params(…)` and `#[param(…)]`.
