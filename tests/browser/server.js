import { copyFileSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawn, spawnSync } from "node:child_process";
const directory = mkdtempSync(join(tmpdir(), "pagelet-browser-"));
if (!process.env.PAGELET_BROWSER_BINARY) {
  const build = spawnSync("cargo", ["build"], { stdio: "inherit" });
  if (build.status !== 0) process.exit(1);
}
// Run a standalone copy outside the checkout: no loose web files are available.
const binary = join(directory, "pagelet");
copyFileSync(
  resolve(process.env.PAGELET_BROWSER_BINARY || "target/debug/pagelet"),
  binary,
);
const init = spawnSync(
  binary,
  ["--data-dir", directory, "init"],
  {
    input: "admin\nbrowser test password\nbrowser test password\n",
    encoding: "utf8",
    cwd: directory,
  },
);
if (init.status !== 0) throw new Error(init.stderr);
const server = spawn(
  binary,
  [
    "--data-dir",
    directory,
    "serve",
    "--listen",
    `127.0.0.1:${process.env.PAGELET_BROWSER_PORT || 18971}`,
  ],
  { stdio: "inherit", cwd: directory },
);
process.on("SIGTERM", () => {
  server.kill("SIGINT");
});
server.on("exit", () => {
  rmSync(directory, { recursive: true, force: true });
  process.exit();
});
