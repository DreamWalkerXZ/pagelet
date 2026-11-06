# Browser tests

Use Node.js 24+ and npm 11+. From the repository root:

```sh
npm ci
npx playwright install chromium
npm run test:browser
```

The suite builds the executable, initializes a real temporary SQLite instance,
starts it on 127.0.0.1:18971, and removes its data on shutdown. The port must be
free. Browser contexts are isolated; no production data or credentials are used.
Tests exercise HTTP APIs and visible browser behavior, without business-rule mocks.
External image loading is aborted at the network boundary in the GFM fixture.

`/_/assets/renderer.js` exports `renderMarkdown(source)`, returning sanitized HTML
for both public reading and editor preview. Assign its return value to the
preview container's `innerHTML`; do not append unsanitized HTML afterwards.

Grant expiry and transitions are covered using controllable server time in the
Rust HTTP suite. Browser tests cover unlock, cookie isolation, raw reads and
password-change revocation without introducing test-only production endpoints.

Editor routes are `/_/app/login`, `/_/app/new`, and
`/_/app/pages/<page-id>/edit`. Editing keeps the last-read version and unsaved
input in memory. Session recovery requires explicit resubmission after login;
reloading a conflicting server version asks before discarding entered work.
The browser recovery test revokes a real session via logout; fixed seven-day
expiry is covered by the controllable-clock Rust HTTP tests.

`/_/assets/api.js` exports `requestJSON(path, { method = 'GET', body } = {})`.
Paths are relative to `/_/api/v1` (for example `/me`). It uses same-origin cookies,
adds the CSRF header, returns parsed JSON (or null for 204), and throws an error
with `status` for HTTP failures. No account API token is used or stored.

Personal management routes are `/_/app/pages` and `/_/app/account`. Lists always
filter by the current owner, including for administrators. Newly issued tokens
live only in the current document and are cleared on dismissal or page exit.
Password changes show the rotated token before offering login again.
Set `PAGELET_BROWSER_PORT` to use a different free test-server port.
The default browser command runs each spec against a fresh temporary instance so
real process-local login limits do not accumulate across independent suites.
Pass a spec filename to run only that spec, for example
`npm run test:browser -- account.spec.js`.

For final embedded release acceptance, run `cargo build --release`, then
`PAGELET_BROWSER_BINARY=target/release/pagelet npm run test:browser`. The selected
binary is copied into the temporary directory and used for both initialization and
serving, with that directory as its working directory and no loose frontend files.
No source checkout or Node.js is needed by the service process.
