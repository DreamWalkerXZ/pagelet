# Pagelet

A self-hosted Markdown publishing service. The initial API slice provides local initialization and bearer account inspection.

## Run

Build with `cargo build --release`. SQLite is bundled into the executable; no database service is required.

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

`GET /_/api/v1/me` returns only the account ID, username, and role. Passwords are stored as salted Argon2id verifiers; random 256-bit API tokens are stored only as SHA-256 hashes. Keep the data directory and token private. Deploy public HTTP behind an HTTPS reverse proxy. Proxy trust and external URL configuration will arrive with the abuse-control slice.

## Persistence and upgrades

The data directory contains `pagelet.sqlite3`. Forward migrations run automatically when the directory opens. Binaries reject newer schema versions; downgrades are unsupported. Before upgrading, stop the service and copy the entire data directory to a separate backup location. Restore a backup only while the service is stopped. Do not rely on copying only the executable.

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
