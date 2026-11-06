import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawn, spawnSync } from "node:child_process";
const directory = mkdtempSync(join(tmpdir(), "pagelet-browser-"));
const build = spawnSync("cargo", ["build"], { stdio: "inherit" });
if (build.status !== 0) process.exit(1);
const init = spawnSync(
  "target/debug/pagelet",
  ["--data-dir", directory, "init"],
  {
    input: "admin\nbrowser test password\nbrowser test password\n",
    encoding: "utf8",
  },
);
if (init.status !== 0) throw new Error(init.stderr);
const server = spawn(
  "target/debug/pagelet",
  [
    "--data-dir",
    directory,
    "serve",
    "--listen",
    `127.0.0.1:${process.env.PAGELET_BROWSER_PORT || 18971}`,
  ],
  { stdio: "inherit" },
);
process.on("SIGTERM", () => {
  server.kill("SIGINT");
});
server.on("exit", () => {
  rmSync(directory, { recursive: true, force: true });
  process.exit();
});
