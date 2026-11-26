# Expose page management over MCP

Pagelet serves MCP over Streamable HTTP at `/_/mcp` by default. Operators can disable the endpoint in the data directory's `config.json`. The same executable serves the web interface, REST API, and MCP. Codex is the initial target client.

Authenticate every MCP request with the existing Pagelet bearer API token. Account permissions and credential revocation apply to each request. The first release uses explicitly configured credentials and leaves OAuth discovery and login outside its scope.

Expose page listing with slug search and pagination, Markdown reads by Page ID, immediate publication, and edits guarded by Page version. Apply the same ownership boundaries and mutation semantics as the REST API. Permanent deletion, ownership transfer, user management, and credential rotation remain outside the first MCP tool set.

Publication takes Markdown and an optional slug. Editing takes a Page ID, the last-read Page version, and one or both of the replacement Markdown and slug. Access password changes remain outside the first MCP tool set.

Return the Page ID, slug, version, and reader link after publication and editing. Build the reader link from the configured external URL when available and return the root-relative reader path otherwise.

The server applies writes when called. Describe publication and edits as write tools so Codex users can configure approval before calls.

Preserve the REST API's machine-readable `slug_taken` and `stale_version` errors in MCP tool results. After a stale-version error, the client should reread the page and reconcile changes before submitting another edit.

Allow requests without an Origin header for native clients. When an Origin header is present, require it to match the explicitly configured site origin. Remote deployments use the existing HTTPS reverse proxy setup.

Use an automated MCP client against the Streamable HTTP endpoint to discover and call all four tools as an ordinary user. Cover cross-owner denial, administrator access, token revocation, preservation of content after a stale edit, default endpoint availability, and disabling the endpoint through configuration. Acceptance relies on simulated client requests and does not require running Codex.
