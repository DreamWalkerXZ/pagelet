import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: "./tests/browser",
  workers: 1,
  use: {
    channel: "chromium",
    baseURL: `http://127.0.0.1:${process.env.PAGELET_BROWSER_PORT || 18971}`,
  },
  webServer: {
    command: "node tests/browser/server.js",
    url: `http://127.0.0.1:${process.env.PAGELET_BROWSER_PORT || 18971}/_/openapi.json`,
    reuseExistingServer: false,
    timeout: 120000,
  },
});
