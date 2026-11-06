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
for both public reading and future editor preview. Assign its return value to the
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
