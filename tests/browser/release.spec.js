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
    expect(html).toContain('rel="icon" type="image/svg+xml" href="/_/assets/favicon.svg"');
    const assets = [...html.matchAll(/(?:src|href)="(\/_\/assets\/[^\"]+)"/g)]
      .map((match) => match[1]);
    expect(assets.length).toBeGreaterThan(0);
    for (const asset of assets) {
      const resource = await request.get(asset);
      expect(resource.status(), asset).toBe(200);
      if (asset === "/_/assets/favicon.svg") {
        expect(resource.headers()["content-type"]).toBe("image/svg+xml");
        expect(await resource.text()).toContain('<svg xmlns="http://www.w3.org/2000/svg"');
      } else {
        expect(resource.headers()["content-type"]).toMatch(/text\/(javascript|css)/);
      }
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
  const quoted = await page.request.post("/_/api/v1/pages", {
    headers,
    data: { slug: "release-quote", markdown: "> # quoted heading\n>\n> first block" },
  });
  expect(quoted.status()).toBe(201);
  for (const [name, path, ready] of [
    ["reader", "/release-layout", "article h1"],
    ["quote", "/release-quote", "article blockquote"],
    ["editor", `/_/app/pages/${record.id}/edit`, "#markdown"],
    ["new", "/_/app/new", "#markdown"],
    ["my-pages", "/_/app/pages", "#rows tr"],
    ["all-pages", "/_/app/admin/pages", "#rows tr"],
    ["login", "/_/app/login", "#username"],
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
      const header = await page.locator("header").boundingBox();
      const heading = await page.locator(name === "quote" ? "article blockquote" : "main h1:visible").first().boundingBox();
      const headerSpacing = await page.locator("header").evaluate(
        (el) => parseFloat(getComputedStyle(el).paddingBottom),
      );
      expect(Math.abs(heading.y - header.y - header.height - headerSpacing)).toBeLessThan(2);
      if (name === "reader") {
        await expect(page.locator("article h1")).toHaveText("Release notes");
      } else if (name === "quote") {
        await expect(page.locator("article h1")).toHaveText("quoted heading");
      } else if (name !== "admin" && name !== "all-pages") {
        await expect(page.locator("footer")).toHaveText("Small Pages. Simply Shared.");
      }
      if (name === "my-pages") {
        const expectEqualGaps = async () => {
          const search = await page.locator(".search-controls").boundingBox();
          const table = await page.locator(".pages-table").boundingBox();
          const pagination = await page.locator("#previous").boundingBox();
          const before = table.y - search.y - search.height;
          expect(before).toBeGreaterThan(0);
          expect(Math.abs(before - (pagination.y - table.y - table.height))).toBeLessThan(2);
        };
        await expectEqualGaps();
        await page.getByLabel("Search slugs").fill("no-such-page");
        await page.getByRole("button", { name: "Search", exact: true }).click();
        await expect(page.getByText("No pages found.")).toBeVisible();
        await expectEqualGaps();
        await page.getByLabel("Search slugs").clear();
        await page.getByRole("button", { name: "Search", exact: true }).click();
        await expect(page.locator("#rows tr").first()).toBeVisible();
      }
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
