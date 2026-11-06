# Pagelet

A self-hosted Markdown publishing service distributed as one Rust executable with embedded browser assets and local SQLite storage. Publish short links, protect reading with a password, manage accounts and pages in the browser, or automate the same operations through the API. No external database or email service is required.

## Run

Build with a Rust toolchain supporting edition 2024 using `cargo build --release`. SQLite and all frontend assets are bundled into the executable. Node.js and loose web files are not required to build or run the service; vendored renderer files are checked in. Copy `target/release/pagelet` to your deployment directory.

```sh
./target/release/pagelet --data-dir ./data init
./target/release/pagelet --data-dir ./data serve
```

Initialization asks for a username, a password, and password confirmation. Password input is hidden on a terminal. Piped input is supported for local automation. Account passwords contain 12–128 Unicode characters, without trimming or normalization. Usernames contain 3–32 lowercase ASCII letters, digits, or single interior hyphens. Save the API token displayed once during initialization. Existing instances cannot be initialized again.

The service refuses to start until initialized. HTTP defaults to `127.0.0.1:8080`. Override the data directory with `--data-dir` or `PAGELET_DATA_DIR`, and the listening address with `serve --listen` or `PAGELET_LISTEN`. CLI arguments take precedence over environment variables.

```sh
curl -H "Authorization: Bearer $PAGELET_TOKEN" http://127.0.0.1:8080/_/api/v1/me
curl http://127.0.0.1:8080/_/openapi.json
```

`GET /_/api/v1/me` returns only the account ID, username, and role. Passwords are stored as salted Argon2id verifiers; random 256-bit API tokens are stored only as SHA-256 hashes. Keep the data directory and token private. Deploy public HTTP behind an HTTPS reverse proxy. Configure the external URL and proxy trust explicitly as described below.

## Browser workflows

Open `http://127.0.0.1:8080/_/app/login` with the initialized administrator account. Create ordinary users or additional administrators under **Users**; give each person their initial password and save any token shown once. There is no public registration. Usernames and roles are fixed; the final administrator cannot be deleted, and a user must own no pages before deletion.

Use **New page** to enter Markdown, inspect the live preview, optionally choose a slug and reading password, then publish. Empty pages publish immediately. **My pages** lists only your own pages, with slug search and pagination. Open a page to edit content, slug or password, or delete it permanently. Concurrent edits return a conflict; keep your unsaved text or explicitly reload the server version. Unsaved text lives in the current document and is lost on navigation or reload.

**Account** changes your password or resets your API token. Copy a newly issued token before leaving or dismissing it: it cannot be retrieved later. Password changes revoke all browser sessions and rotate the token; token reset alone preserves sessions. **Log out** ends the current browser session.

Administrators use **All pages** and **Users** to search and paginate the instance, filter pages by owner ID, transfer ownership, reset credentials and delete accounts/pages. Transfers require the latest version; the old owner immediately loses management privileges. Administrators are trusted: they can read and change every page, including password-protected pages, reset credentials and transfer ownership. A page access password does not hide content from administrators or the operator.

Share `/<slug>` for reading. Visitors enter a page password when required; a page-specific HttpOnly cookie allows 24 hours of reading, including **Raw Markdown**. Owners and administrators bypass the password. Reader and editor preview share sanitized Markdown rendering: GFM tables, display-only task lists, strikethrough, autolinks and fenced code blocks. Raw HTML and dangerous link schemes are disabled. Images use external HTTP(S) URLs and contact their hosts when loaded; no image upload or proxy is provided.

Reading and management require JavaScript. There is no server-rendered article, search-engine/social-preview support, or public page directory. There is no separate title or draft state, revision history, recycle bin, syntax highlighting or collaborative editing.

| Route | Purpose |
| --- | --- |
| `/<slug>` | Public reader shell; protected content still requires authorization |
| `/_/raw/<slug>` | UTF-8 Markdown source with the same reading policy |
| `/_/app/login`, `/_/app/new` | Login and publication |
| `/_/app/pages`, `/_/app/pages/<id>/edit` | Personal list and stable-ID editor |
| `/_/app/account` | Password, token and logout |
| `/_/app/admin/pages`, `/_/app/admin/users` | Administrator management |
| `/_/api/v1/*` | Versioned JSON API |
| `/_/openapi.json` | Public API contract |
| `/_/assets/*` | Embedded scripts, styles and renderer dependencies |

Changing or deleting a slug releases it immediately without a redirect or reservation. Old links may later display someone else's page. Page deletion is permanent at the application level, including password verifiers and grants, but cannot recall downloaded content or erase historical backups/storage media. Account deletion revokes its credentials. Keep private backups and treat shared links accordingly.

## Persistence and upgrades

The data directory contains `pagelet.sqlite3`. Forward migrations run automatically when the directory opens. Binaries reject newer schema versions; downgrades are unsupported. Before upgrading, stop the service and copy the entire data directory to a separate backup location. Restore a backup only while the service is stopped. Do not rely on copying only the executable. The executable contains the service and SQLite engine, not your accounts, credentials, pages, or access grants. No runtime database service or source checkout is required.

Use absolute paths for service deployment and restrict the data directory to the service account. For a local example, stop Pagelet (and disable automatic restart), then run:

```sh
# Run only after the service has stopped. Keep this backup before every upgrade.
cp -a /srv/pagelet/data /srv/backups/pagelet-before-upgrade
# Replace the executable, then restart with the same data directory.
/srv/pagelet/pagelet --data-dir /srv/pagelet/data serve
```

Copy the **entire** directory, including any SQLite journal or sidecar files. Choose a new backup destination each time; keep backups private because they contain account verifiers and page content. An online file copy is not a supported backup method. Verify the copied backup by starting it separately on a loopback test port and checking authenticated API/raw reads, then stop that test instance.

To restore, stop Pagelet again, move the current data directory aside, and copy the chosen backup back to the configured path. Preserve ownership and permissions, then start a compatible executable and verify account authentication and representative pages before reopening public traffic. Never overlay a backup onto an active or partially populated directory. A backup restores the whole instance as of that copy, including credentials and grants; later edits and credential rotations will be lost. Keep the previous binary with its pre-upgrade backup if rollback is needed: do not open an upgraded database with an older binary. Migrations run before serving requests, and a newer unsupported schema is refused without attempting a downgrade.

## Verify

Run `cargo test` for executable CLI and real temporary SQLite/HTTP integration checks. `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` check formatting and compiler lints.

Administrators can provision accounts with `POST /_/api/v1/users` using a bearer
API token and a JSON body containing `username`, `password`, and `role`
(`administrator` or `user`). The 201 response contains `user` metadata and its
one-time `token`; save the token then, as account reads never disclose it.
Usernames and roles cannot be changed. Duplicate usernames return 409; invalid
credentials or roles return 400. Passwords preserve Unicode and spaces.

`GET /_/api/v1/users` is administrator-only and returns
`{users, total, page, per_page}`. It supports literal username substring `search`,
1-based `page`, and `per_page` (20 by default, capped at 100). Results sort by
username and ID. Missing credentials return 401; ordinary users receive 403.
The public `/_/openapi.json` describes both operations and their schemas.

Browser login uses `POST /_/api/v1/login` with JSON `username` and `password`.
Send `X-Pagelet-CSRF: 1` on login and every cookie-authenticated mutation.
The API deliberately provides no cross-origin CORS permission: this custom header
requires a same-origin browser request. The `pagelet_session` cookie is HttpOnly,
host-only, SameSite=Lax, Path=/, and expires after exactly seven days without renewal.
`POST /_/api/v1/logout` revokes only the current browser session. An explicitly
supplied invalid bearer token never falls back to a browser cookie.

`POST /_/api/v1/me/password` accepts `current_password` and a new `password`.
Administrators reset an account password at `POST /_/api/v1/users/{id}/password`
with `password`. Both return the replacement API `token` once and revoke all of
that account's browser sessions. `POST /_/api/v1/me/token` or
`POST /_/api/v1/users/{id}/token` (self or administrator) rotates only the API token,
preserving browser sessions. API tokens otherwise do not expire.

For local administrator recovery, run `pagelet --data-dir ./data recover-admin`.
It asks for the administrator username, a hidden new password, and confirmation,
then displays the replacement API token once. Ordinary accounts cannot use this
recovery command. Recovery also invalidates every session and old API token.

Login allows 10 attempts per 60-second window independently per effective source IP
and target account, including successful attempts. Further attempts receive 429;
the window expires without permanent account lockout. Counters are process-local
and reset on service restart. Direct clients cannot override their IP using
forwarding headers.

Authenticated users publish immediately with `POST /_/api/v1/pages`, sending
`{"markdown":"# Hello","slug":"hello"}`. Omit `slug` for a generated address;
empty Markdown is allowed. Custom slugs use 1–64 lowercase ASCII letters, digits,
and single interior hyphens. Occupied custom slugs return 409. Markdown is limited
to 1 MiB of UTF-8 bytes; JSON request bodies are bounded at 8 MiB to allow escaping.

Read source and metadata at `GET /_/api/v1/pages/by-slug/hello`, or plain Markdown
at `GET /_/raw/hello`. Content responses disallow caching. Page IDs are immutable;
timestamps are Unix milliseconds and initial versions are 1. There is no rendered
HTML endpoint. `GET /_/api/v1/pages` requires account authentication and returns
metadata only, with `search`, `page` (default 1), and `per_page` (default 20,
capped at 100). Ordinary users see their pages; administrators see all and can
filter by `owner_id`. Results use descending update time, then descending ID.

Page management uses the immutable `id` from publication or management lists:
`GET /_/api/v1/pages/{id}`, `PATCH /_/api/v1/pages/{id}` with
`{"version":1,"markdown":"# Updated","slug":"new-address"}`, and
`DELETE /_/api/v1/pages/{id}` with `{"version":2}`. Authenticate as the owner
or an administrator. PATCH accepts either or both Markdown and slug; every successful
change advances the version and update timestamp. Supply the last-read version;
stale edits and deletions return 409 without modifying the page.

Changing a slug preserves page identity, ownership, content, and access policy.
The previous slug immediately becomes reusable and has no redirect. Deletion
permanently removes the page and releases its slug, with no recycle bin or restore
API; this does not guarantee secure erasure from storage or backups.

Page access passwords can be set during creation or changed with `PATCH /_/api/v1/pages/{id}` using `access_password` and the last-read `version`. Passwords contain 1–128 Unicode characters without trimming or normalization; `""` removes protection. Omit the field to preserve the current policy. Password verifiers are never returned. Owners and administrators bypass protection; other accounts have the same read access as visitors.

Protected API and raw reads return HTTP 401 with only `{"error":"password_required"}` until authorized. Submit `{"password":"..."}` to `POST /_/api/v1/pages/by-slug/{slug}/verify-password`. Successful verification returns `{"grant":"...","expires_at":...}` (Unix seconds) and an HttpOnly `pagelet_grant_{page_id}` cookie with `Path=/`, `SameSite=Lax`, and `Max-Age=86400`. Scripts send the grant in `X-Pagelet-Grant`; URLs never carry credentials. Multiple pages use separate cookies. Grants authorize reading only and expire after a fixed 24 hours, including across service restarts. Password changes/removal, page deletion, and ownership transfer revoke grants. Grants follow immutable page identity, so reusing a slug cannot reuse its authorization. Reads and grant issuance use `Cache-Control: no-store`; policy changes cannot recall content already downloaded.

Page-password verification reuses login abuse controls: at most ten attempts per effective source IP and per page in a 60-second window, with HTTP 429 on excess attempts. Counters are process-local and reset on restart. Forwarding headers from direct clients cannot bypass these limits.


## HTTPS reverse proxy deployment

Pagelet serves HTTP only; terminate TLS at the reverse proxy. Set
`serve --external-url https://pages.example --trusted-proxy 127.0.0.1`
(or `PAGELET_EXTERNAL_URL` and comma-separated `PAGELET_TRUSTED_PROXIES`).
Repeat `--trusted-proxy` or supply comma-separated IPs/CIDRs, including IPv6.
Trust no proxies by default. Trust only ingress and internal proxy addresses you
control; do not trust the entire internet. Keep the backend unreachable by public
clients. Configuration is validated at startup, and CLI flags override environment
values.

For nginx terminating HTTPS on the same host, place this in its TLS server:

```nginx
location / {
    proxy_pass http://127.0.0.1:8080;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-For $remote_addr;
}
```

Or use Caddy (which supplies the forwarding header) with the same Pagelet flags:

```caddyfile
pages.example {
    reverse_proxy 127.0.0.1:8080
}
```

The public ingress must overwrite client-supplied `X-Forwarded-For`, or correctly
append its connected peer. Internal proxies must correctly append their peer.
Pagelet ignores forwarding headers from untrusted connected peers. For trusted
peers it validates every comma-separated address in all header fields, then walks
right-to-left, skipping trusted proxies and stopping at the first untrusted IP.
A missing header falls back to the connected peer. Malformed entries (including
empty entries, ports, and `unknown`) return 400 for login/password verification;
entries are never silently skipped. Both source and target limits still apply.

An HTTPS external URL adds `Secure` to session and page access grant cookies,
including session removal on logout. HTTP external URLs (or no configured URL)
keep local HTTP testing possible. Forwarded scheme/host headers cannot change
cookie policy. Keep browser requests on the same external origin, preserve
`X-Pagelet-CSRF: 1` on login and cookie-authenticated mutations, and do not add
permissive CORS headers at the proxy. TLS termination does not replace CSRF
protection. The external URL is deployment configuration, not built-in TLS or a
redirect service.

## Release acceptance

`cargo test --test acceptance` runs the actual executable with a real temporary SQLite directory through initialization, user provisioning, publication/raw reading, password verification, token rotation, ownership transfer, and permanent account/page deletion. It stops the process, copies the entire current data directory, and runs the restored instance from the copy. Requests and observed responses are checked against the public OpenAPI operation's status, media type, and structural schemas (references, required fields, types, enums, and constants), including conflicts and rate limits. This is a focused contract regression check, not a general OpenAPI/JSON Schema conformance engine.

The remaining HTTP/SQLite suites cover session/CSRF and authorization alternatives, validation boundaries, concurrency, proxy abuse controls, credential revocation, and forward migration fixtures for schema versions 1–3. CLI tests cover uninitialized startup refusal, recovery, repeat initialization refusal, and newer-schema rejection. Run the full suite for release acceptance:

```sh
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo build --release
```

Visitors cannot enumerate users/pages or publish anonymously. There is no public registration, role editing, account suspension, backend HTML rendering, attachment storage, online backup, or database downgrade. The API and embedded browser delivery phases are complete.

Browser acceptance uses Node.js 24+, npm 11+ and Chromium only as development tools:

```sh
npm ci
npx playwright install chromium
PAGELET_BROWSER_BINARY=target/release/pagelet npm run test:browser
```

The browser runner copies the selected executable into a fresh temporary directory and runs it there, without loose frontend files. Each spec gets a fresh instance to isolate process-local rate limits. Omit `PAGELET_BROWSER_BINARY` to build and test the debug executable; use `PAGELET_BROWSER_PORT` to select a free port (default 18971). Browser suites cover reader/GFM sanitization, password grants/raw reads, editing/conflicts, account isolation, credential rotation, administrator transfer/deletion and embedded routes/assets. See [browser test notes](tests/browser/README.md).

No performance threshold, multi-instance coordination, online backup or built-in TLS is promised. Rate limits reset on restart. Run one service process per data directory.

## License

Pagelet is licensed under the [MIT License](LICENSE), copyright 2026 DreamWalkerXZ. Vendored frontend dependencies retain their own notices in [web/vendor](web/vendor): Marked (MIT) and DOMPurify (Apache-2.0 OR MPL-2.0). Preserve these notices when redistributing the executable and sources.
