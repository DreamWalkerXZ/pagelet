### Upgrade from v0.1.0

Stop Pagelet and back up the entire data directory before starting v0.2.0.
Startup automatically migrates the database to schema 5 and preserves existing
user IDs. After the migration, deleted user IDs are no longer reassigned.
To roll back to v0.1.0, restore the pre-upgrade backup before starting the old
binary.

### Changes

- Confirm API token resets before replacing the old token. The new token dialog
  explains that the value can only be viewed once.
- Confirm slug changes and access password removal. The editor keeps unsaved
  work on conflicts and offers to reload only when the page version is stale.
- Search by username to select a new page owner. A transfer to the current owner
  returns HTTP 400 and preserves existing visitor access grants.
- Page conflicts still return HTTP 409, now with a JSON `code` of
  `slug_taken` or `stale_version`.
- Restore Markdown H2 styling in Live Preview and add a favicon.

### Standalone downloads

The archives were smoke-tested on Linux x86_64 and arm64 (static musl builds)
and macOS arm64. Compare your archive's SHA-256 digest with `SHA256SUMS` before
extracting it. Run `./pagelet init`, then `./pagelet serve`. Open
`http://127.0.0.1:8080` and sign in with the administrator you created.

### Docker

The verified Linux amd64 and arm64 image is published to Docker Hub as
`dreamwalkerxz/pagelet:0.2.0` and `dreamwalkerxz/pagelet:latest` after this release
is published. Mount a persistent volume at `/data` for both commands.

```sh
docker volume create pagelet-data
docker run --rm -it -v pagelet-data:/data dreamwalkerxz/pagelet:0.2.0 init
docker run -d --name pagelet -p 127.0.0.1:8080:8080 \
  -v pagelet-data:/data dreamwalkerxz/pagelet:0.2.0
```

The `pagelet-image.oci.tar` release asset contains both verified image platforms
and is the source for Docker Hub publication.

For unattended first startup with either distribution, set
`PAGELET_ADMIN_USERNAME` and `PAGELET_ADMIN_PASSWORD` in the server environment
and skip `init`. After signing in, use **Account → Reset API Token** if you
need an API token.
