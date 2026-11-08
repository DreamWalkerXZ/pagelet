# Initialize locally and deploy behind an HTTPS proxy

Run `pagelet init` to interactively create the first administrator with hidden password input and a one-time API token display, then `pagelet serve` to start the service. For unattended startup, `serve` can create the first administrator from `PAGELET_ADMIN_USERNAME` and `PAGELET_ADMIN_PASSWORD` using the same validation and transaction. A new instance requires both variables for this path and names the missing variable if only one is set. With neither variable, `serve` requires local initialization. There are no default credentials or public web initialization endpoints.

An initialized instance ignores both variables. Automatic initialization logs the source at INFO level and keeps all credentials private. The administrator can obtain a token by signing in and using **Account → Reset API Token**. Local administrator recovery remains available.

Serve HTTP on `127.0.0.1:8080` by default, configurable through CLI arguments or environment variables. Public deployments terminate HTTPS at a reverse proxy, with explicit configuration of the external URL and trusted proxy boundary rather than built-in certificate management.
