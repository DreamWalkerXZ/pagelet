import { readdirSync } from "node:fs";
import { spawnSync } from "node:child_process";

// Each spec gets a fresh real server so login rate limits do not leak across suites.
const filters = process.argv.slice(2);
const specs = readdirSync("tests/browser").filter((name) =>
  name.endsWith(".spec.js"),
);
for (const spec of filters.length ? [null] : specs) {
  const result = spawnSync(
    process.execPath,
    [
      "node_modules/@playwright/test/cli.js",
      "test",
      ...(spec ? [spec] : filters),
    ],
    { stdio: "inherit" },
  );
  if (result.status !== 0) process.exit(result.status ?? 1);
}
