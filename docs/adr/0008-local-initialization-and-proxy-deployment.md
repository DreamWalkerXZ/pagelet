# Initialize locally and deploy behind an HTTPS proxy

Run `pagelet init` to interactively create the first administrator with hidden password input and a one-time API token display, then `pagelet serve` to start the service. Refuse to serve an uninitialized instance, with no default credentials or public web initialization endpoint.

Serve HTTP on `127.0.0.1:8080` by default, configurable through CLI arguments or environment variables. Public deployments terminate HTTPS at a reverse proxy, with explicit configuration of the external URL and trusted proxy boundary rather than built-in certificate management.
