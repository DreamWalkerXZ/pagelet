import { test, expect } from "@playwright/test";

test("embedded delivery serves every web entry point and its local resources", async ({
  request,
}) => {
  const contract = await request.get("/_/openapi.json");
  expect(contract.status()).toBe(200);
  expect((await contract.json()).openapi).toMatch(/^3\./);
  expect((await request.get("/_/api/v1/me")).status()).toBe(401);
  await request.post("/_/api/v1/login", { headers: { "x-pagelet-csrf": "1" }, data: { username: "admin", password: "browser test password" } });
  for (const path of [
    "/_/app/login", "/_/app/new", "/_/app/pages", "/_/app/account",
    "/_/app/admin/pages", "/_/app/admin/users", "/_/app/pages/1/edit",
    "/release-note",
  ]) {
    const response = await request.get(path);
    expect(response.status()).toBe(200);
    expect(response.headers()["content-type"]).toContain("text/html");
    const html = await response.text();
    const assets = [...html.matchAll(/(?:src|href)="(\/_\/assets\/[^\"]+)"/g)]
      .map((match) => match[1]);
    expect(assets.length).toBeGreaterThan(0);
    for (const asset of assets) {
      const resource = await request.get(asset);
      expect(resource.status(), asset).toBe(200);
      expect(resource.headers()["content-type"]).toMatch(/text\/(javascript|css)/);
    }
  }
  expect((await request.get("/_/raw/release-note")).status()).toBe(404);
});

test("reader, editor, account and administrator screens remain usable at desktop and mobile widths", async ({
  page,
}) => {
  const headers = { "x-pagelet-csrf": "1" };
  await page.request.post("/_/api/v1/login", {
    headers,
    data: { username: "admin", password: "browser test password" },
  });
  const created = await page.request.post("/_/api/v1/pages", {
    headers,
    data: {
      slug: "release-layout",
      markdown: "# Release notes\n\nA small place for useful pages.\n\n| Feature | State |\n| --- | --- |\n| Embedded web | Ready |\n\n- [x] Share a page\n- [ ] Write the next one",
    },
  });
  expect(created.status()).toBe(201);
  const record = await created.json();
  for (const [name, path, ready] of [
    ["reader", "/release-layout", "article h1"],
    ["editor", `/_/app/pages/${record.id}/edit`, "#markdown"],
    ["account", "/_/app/account", "#current-password"],
    ["admin", "/_/app/admin/users", "#rows tr"],
  ]) {
    for (const [size, width, height] of [
      ["desktop", 1440, 1000], ["mobile", 390, 844],
    ]) {
      await page.setViewportSize({ width, height });
      await page.goto(path);
      await expect(page.locator(ready).first()).toBeVisible();
      expect(await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      )).toBeTruthy();
      if (name === "editor") {
        await expect(page.locator("#preview input[type=checkbox]").first())
          .toHaveCSS("display", "inline-block");
      }
      await page.screenshot({
        path: `output/playwright/release-${name}-${size}.png`,
        fullPage: true,
      });
    }
  }
});
