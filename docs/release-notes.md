Pagelet is a self-hosted Markdown publishing service in one executable. Sign in to
publish GitHub Flavored Markdown at short URLs, edit pages with a live preview,
share raw Markdown, or require a password to read a page. Manage pages and users
in the browser or through the OpenAPI-described API. The browser assets and
SQLite engine are built into the executable.

### Standalone downloads

The archives were smoke-tested on Linux x86_64 and arm64 (static musl builds)
and macOS arm64. Compare your archive's SHA-256 digest with `SHA256SUMS` before
extracting it. Run `./pagelet init`, then `./pagelet serve`. Open
`http://127.0.0.1:8080` and sign in with the administrator you created.

### Docker

The verified Linux amd64 and arm64 image is published to Docker Hub as
`dreamwalkerxz/pagelet:0.1.0` and `dreamwalkerxz/pagelet:latest` after this release
is published. Mount a persistent volume at `/data` for both commands.

```sh
docker volume create pagelet-data
docker run --rm -it -v pagelet-data:/data dreamwalkerxz/pagelet:0.1.0 init
docker run -d --name pagelet -p 127.0.0.1:8080:8080 \
  -v pagelet-data:/data dreamwalkerxz/pagelet:0.1.0
```

The `pagelet-image.oci.tar` release asset contains both verified image platforms
and is the source for Docker Hub publication.

For unattended first startup with either distribution, set
`PAGELET_ADMIN_USERNAME` and `PAGELET_ADMIN_PASSWORD` in the server environment
and skip `init`. After signing in, use **Account → Reset API Token** if you
need an API token.
