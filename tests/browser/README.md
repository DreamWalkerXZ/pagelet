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
