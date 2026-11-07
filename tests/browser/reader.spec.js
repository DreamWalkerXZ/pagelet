import { test, expect } from "@playwright/test";

test("reads safe GFM using only embedded application assets", async ({
  page,
  request,
  baseURL,
}) => {
  const login = await request.post("/_/api/v1/login", {
    headers: { "x-pagelet-csrf": "1" },
    data: { username: "admin", password: "browser test password" },
  });
  expect(login.ok()).toBeTruthy();
  const markdown =
    '# Field notes\n\n| Plant | Count |\n| --- | --- |\n| Fern | 2 |\n\n- [x] Complete\n- [ ] Pending\n\n~~old~~ https://example.com\n\n```js\nconst x = 1;\n```\n\n<script>window.compromised=true</script><img src=x onerror="window.compromised=true">\n\n[unsafe](javascript:alert%281%29)\n\n![external](https://example.com/plant.png)';
  const create = await request.post("/_/api/v1/pages", {
    headers: { "x-pagelet-csrf": "1" },
    data: { slug: "field-notes", markdown },
  });
  expect(create.status()).toBe(201);
  const external = [];
  page.on("request", (r) => {
    if (!r.url().startsWith(baseURL)) external.push(r.url());
  });
  await page.route("https://example.com/plant.png", (route) => route.abort());
  await page.goto("/field-notes");
  await expect(
    page.getByRole("heading", { name: "Field notes" }),
  ).toBeVisible();
  await expect(page.getByRole("cell", { name: "Fern" })).toBeVisible();
  await expect(page.getByRole("checkbox")).toHaveCount(2);
  await expect(page.getByRole("checkbox").first()).toBeChecked();
  await expect(page.getByRole("checkbox").first()).toBeDisabled();
  await expect(page.locator("del")).toHaveText("old");
  await expect(
    page.getByRole("link", { name: "https://example.com" }),
  ).toHaveAttribute("href", "https://example.com");
  await expect(page.locator("pre code")).toHaveText("const x = 1;\n");
  expect(await page.evaluate(() => window.compromised)).toBeUndefined();
  await expect(
    page.locator(
      'article script, article [onerror], article a[href^="javascript:"]',
    ),
  ).toHaveCount(0);
  expect(external).toEqual(["https://example.com/plant.png"]);
  const raw = await page
    .getByRole("link", { name: "Raw Markdown" })
    .getAttribute("href");
  expect(await (await page.request.get(raw)).text()).toBe(markdown);
});

test("protected reading grants raw access and returns to the challenge after revocation", async ({
  page,
  request,
  context,
}) => {
  await request.post("/_/api/v1/login", {
    headers: { "x-pagelet-csrf": "1" },
    data: { username: "admin", password: "browser test password" },
  });
  const created = await request.post("/_/api/v1/pages", {
    headers: { "x-pagelet-csrf": "1" },
    data: {
      slug: "private-note",
      markdown: "# Confidential fern",
      access_password: "open sesame",
    },
  });
  const record = await created.json();
  await page.goto("/private-note");
  await expect(
    page.getByRole("heading", { name: "A password opens this page." }),
  ).toBeVisible();
  await expect(page.getByRole("link", { name: "Raw Markdown" })).toBeHidden();
  expect(await page.locator("body").textContent()).not.toContain(
    "Confidential fern",
  );
  await page.getByLabel("Access password", { exact: true }).fill("wrong");
  await page.getByRole("button", { name: "Open page" }).click();
  await expect(page.getByRole("alert")).toHaveText(
    "Incorrect password. Please try again.",
  );
  await page.getByLabel("Access password", { exact: true }).fill("open sesame");
  await page.getByRole("button", { name: "Open page" }).click();
  await expect(
    page.getByRole("heading", { name: "Confidential fern" }),
  ).toBeVisible();
  const grant = (await context.cookies()).find((c) =>
    c.name.startsWith("pagelet_grant_"),
  );
  expect(grant.httpOnly).toBeTruthy();
  expect(grant.expires * 1000 - Date.now()).toBeGreaterThan(86300000);
  expect(
    await page.evaluate(() => ({
      cookie: document.cookie,
      local: localStorage.length,
      session: sessionStorage.length,
    })),
  ).toEqual({ cookie: "", local: 0, session: 0 });
  const raw = await page.request.get("/_/raw/private-note");
  expect(raw.ok()).toBeTruthy();
  expect(raw.headers()["content-type"]).toBe("text/plain; charset=utf-8");
  expect(await raw.text()).toBe("# Confidential fern");
  const changed = await request.patch(`/_/api/v1/pages/${record.id}`, {
    headers: { "x-pagelet-csrf": "1" },
    data: { version: record.version, access_password: "changed password" },
  });
  expect(changed.ok()).toBeTruthy();
  await page.reload();
  await expect(
    page.getByRole("heading", { name: "A password opens this page." }),
  ).toBeVisible();
  expect((await page.request.get("/_/raw/private-note")).status()).toBe(401);
  await page.goto("/missing-page");
  await expect(page.getByRole("alert")).toHaveText("Page not found.");
});

test("reserves service routes and rejects unknown assets without an HTML fallback", async ({
  request,
}) => {
  const reader = await request.get("/valid-slug");
  expect(reader.headers()["content-type"]).toBe("text/html; charset=utf-8");
  for (const path of [
    "/_/assets/reader.js",
    "/_/assets/renderer.js",
    "/_/assets/vendor/marked.js",
    "/_/assets/vendor/purify.js",
  ]) {
    const response = await request.get(path);
    expect(response.status()).toBe(200);
    expect(response.headers()["content-type"]).toBe(
      "text/javascript; charset=utf-8",
    );
    expect(response.headers()["x-content-type-options"]).toBe("nosniff");
  }
  for (const path of [
    "/_/assets/missing.js",
    "/_/app/future",
    "/INVALID",
    "/bad--slug",
  ]) {
    expect((await request.get(path)).status()).toBe(404);
  }
});
