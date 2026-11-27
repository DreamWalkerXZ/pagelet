### MCP page management

- Pagelet serves MCP over Streamable HTTP at `/_/mcp` by default.
- Connect with an existing Pagelet API token to list and search manageable
  pages, read Markdown by page ID, publish pages, and edit them with a version
  check. Existing ownership rules apply to each request.
- To disable MCP, set `{"mcp": false}` in `config.json` in the Pagelet data
  directory and restart the service. Docker uses `/data/config.json`.
- See the [MCP configuration guide](https://github.com/DreamWalkerXZ/pagelet/blob/v0.3.0/README.md#mcp)
  for Codex setup and write approvals.

### Upgrade from v0.2.0

Keep the existing data directory when restarting with the new binary or image.
This release uses the same database schema as v0.2.0.

### Standalone downloads

The archives were smoke-tested on Linux x86_64 and arm64 (static musl builds)
and macOS arm64. Compare your archive's SHA-256 digest with `SHA256SUMS` before
extracting it. Run `./pagelet init`, then `./pagelet serve`. Open
`http://127.0.0.1:8080` and sign in with the administrator you created.

### Docker

The verified Linux amd64 and arm64 image is published to Docker Hub as
`dreamwalkerxz/pagelet:0.3.0` and `dreamwalkerxz/pagelet:latest` after this release
is published. Mount a persistent volume at `/data` for both commands.

```sh
docker volume create pagelet-data
docker run --rm -it -v pagelet-data:/data dreamwalkerxz/pagelet:0.3.0 init
docker run -d --name pagelet -p 127.0.0.1:8080:8080 \
  -v pagelet-data:/data dreamwalkerxz/pagelet:0.3.0
```

The `pagelet-image.oci.tar` release asset contains both verified image platforms
and is the source for Docker Hub publication.

For unattended first startup with either distribution, set
`PAGELET_ADMIN_USERNAME` and `PAGELET_ADMIN_PASSWORD` in the server environment
and skip `init`. After signing in, use **Account → Reset API Token** if you
need an API token.
