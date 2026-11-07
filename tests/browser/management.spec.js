import { expect, test } from "@playwright/test";

test("home and protected management views lead to My pages after login", async ({ page }) => {
  const home = await page.request.get("/", { maxRedirects: 0 });
  expect(home.headers().location).toBe("/_/app/pages");
  await page.goto("/_/app/admin/pages");
  await expect(page).toHaveURL(/\/_\/app\/login$/);
  await page.getByLabel("Username", { exact: true }).fill("admin");
  await page.getByLabel("Account password").fill("browser test password");
  await page.getByRole("button", { name: "Log in" }).click();
  await expect(page).toHaveURL(/\/_\/app\/pages$/);
  const nav = page.getByRole("navigation", { name: "Management" });
  for (const path of ["pages", "new", "account", "admin/pages", "admin/users"]) {
    await page.goto("/_/app/" + path);
    await expect(nav.getByRole("link")).toHaveText(["New page", "My pages", "Account", "All pages", "Users"]);
  }
});

test("new and edit previews have bounded equal panes with desktop source-block scrolling", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.request.post("/_/api/v1/login", {
    headers: { "x-pagelet-csrf": "1" },
    data: { username: "admin", password: "browser test password" },
  });
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/_/app/new");
  const source = page.getByLabel("Markdown");
  const preview = page.getByLabel("Page preview");
  const markdown = Array.from({ length: 30 }, (_, i) =>
    "# Section " + i + "\n\n| Column | Data |\n| --- | --- |\n| Value | More |",
  ).join("\n\n");
  await source.fill(markdown);
  await expect(preview.getByRole("table")).toHaveCount(30);
  for (const route of ["new", "edit"]) {
    if (route === "edit") {
      await page.getByLabel("Slug", { exact: true }).fill("scroll-preview");
      await page.getByRole("button", { name: "Publish page" }).click();
      await expect(page).toHaveURL(/\/edit$/);
      await page.getByRole("button", { name: "Copy link" }).click();
      expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(new URL("/scroll-preview", page.url()).href);
    }
    await expect(page.getByText("Publishing is immediate.", { exact: false })).toHaveCount(0);
    expect((await source.boundingBox()).height).toBe((await preview.boundingBox()).height);
    await source.evaluate((el) => { el.scrollTop = 0; });
    await expect.poll(() => preview.evaluate((el) => el.scrollTop)).toBe(0);
    const sourceTop = await source.evaluate((el) => {
      const line = el.value.slice(0, el.value.indexOf("# Section 10\n")).split("\n").length - 1;
      const style = getComputedStyle(el);
      el.scrollTop = line * parseFloat(style.lineHeight) + parseFloat(style.paddingTop);
      return el.scrollTop;
    });
    expect(sourceTop).toBeGreaterThan(400);
    await expect.poll(async () => {
      const heading = await preview.getByRole("heading", { name: "Section 10", exact: true }).boundingBox();
      return Math.abs(heading.y - (await preview.boundingBox()).y);
    }).toBeLessThan(8);
    const previous = await source.evaluate((el) => el.scrollTop);
    await preview.evaluate((el) => { el.scrollTop += 350; });
    await expect.poll(() => source.evaluate((el) => el.scrollTop)).toBeGreaterThan(previous + 40);
  }
  await page.setViewportSize({ width: 390, height: 844 });
  expect((await source.boundingBox()).height).toBe((await preview.boundingBox()).height);
  const previous = await preview.evaluate((el) => el.scrollTop);
  await source.evaluate((el) => { el.scrollTop += 200; });
  await expect.poll(() => source.evaluate((el) => el.scrollTop)).toBeGreaterThan(0);
  expect(await preview.evaluate((el) => el.scrollTop)).toBe(previous);
});

test("typing inside a tall source block keeps its short rendered block visible", async ({ page }) => {
  await page.request.post("/_/api/v1/login", {
    headers: { "x-pagelet-csrf": "1" },
    data: { username: "admin", password: "browser test password" },
  });
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/_/app/new");
  const source = page.getByLabel("Markdown");
  const link = "[link](https://example.com/" + "a".repeat(12000) + ")";
  await source.fill(link + "\n\n" + Array.from({ length: 40 }, (_, i) => "# Heading " + i).join("\n\n"));
  await source.evaluate((el, offset) => el.setSelectionRange(offset, offset), link.length - 1);
  await source.press("x");
  const preview = page.getByLabel("Page preview");
  await expect.poll(async () => {
    const block = await preview.getByRole("link", { name: "link" }).boundingBox();
    const pane = await preview.boundingBox();
    return block.y >= pane.y && block.y + block.height <= pane.y + pane.height;
  }).toBe(true);
});
