import { test, expect } from "@playwright/test";

test("logs in, publishes and previews a page through its stable editor URL", async ({
  page,
  context,
  browser,
}) => {
  await page.goto("/_/app/new");
  await page.getByLabel("Username").fill("admin");
  await page.getByLabel("Account password").fill("browser test password");
  await page.getByRole("button", { name: "Log In" }).click();
  await expect(page).toHaveURL(/\/_\/app\/pages$/);
  await page.getByRole("link", { name: "New Page", exact: true }).click();
  await page
    .getByLabel("Markdown")
    .fill("# Editor fern\n\n## Fern notes\n\n~~old~~\n\n- [x] Complete");
  await expect(
    page.getByRole("heading", { name: "Editor fern" }),
  ).toBeVisible();
  await expect(page.locator("#preview del")).toHaveText("old");
  await page.getByLabel("Slug", { exact: true }).fill("editor-fern");
  await page.getByLabel("Access password", { exact: true }).fill("fern secret");
  await page.getByRole("button", { name: "Publish Page" }).click();
  await expect(page).toHaveURL(/\/_\/app\/pages\/\d+\/edit$/);
  await expect(page.getByRole("status")).toHaveText("Page published.");
  const editorURL = page.url();
  const preview = await page.locator("#preview").textContent();
  await page.getByRole("link", { name: "Visit Page" }).click();
  await expect(page.locator("article")).toHaveText(preview);
  const headingSize = await page.getByRole("heading", { name: "Fern notes", level: 2 })
    .evaluate((heading) => getComputedStyle(heading).fontSize);
  await page.goto(editorURL);
  const source = page.getByLabel("Markdown");
  await source.fill((await source.inputValue()).replace("## Fern notes", "## Updated fern notes"));
  await expect(page.getByLabel("Page Preview").getByRole("heading", {
    name: "Updated fern notes", level: 2,
  })).toHaveCSS("font-size", headingSize);
  const save = page.getByRole("button", { name: "Save Changes", exact: true });
  const confirmation = page.getByRole("dialog", { name: "Save Changes" });
  await page.getByLabel("Slug", { exact: true }).fill("editor-fern-renamed");
  await page.getByLabel("Access Password Action").selectOption("replace");
  await save.click();
  await expect(page.getByRole("alert")).toContainText("Enter a new access password");
  await page
    .getByLabel("Access password", { exact: true })
    .fill("new fern secret");
  await save.click();
  await expect(confirmation).toContainText("will not redirect");
  await confirmation.getByRole("button", { name: "Save Changes" }).click();
  await expect(page.getByRole("status")).toHaveText("Changes saved.");
  await expect(page.getByRole("link", { name: "Visit Page" })).toHaveAttribute(
    "href",
    "/editor-fern-renamed",
  );
  const visitor = await browser.newContext();
  const readPage = await visitor.newPage();
  await readPage.goto("/editor-fern-renamed");
  await readPage
    .getByLabel("Access password", { exact: true })
    .fill("fern secret");
  await readPage.getByRole("button", { name: "Open Page" }).click();
  await expect(readPage.getByRole("alert")).toContainText("Incorrect password");
  await readPage
    .getByLabel("Access password", { exact: true })
    .fill("new fern secret");
  await readPage.getByRole("button", { name: "Open Page" }).click();
  await expect(
    readPage.getByRole("heading", { name: "Editor fern" }),
  ).toBeVisible();
  await page.getByLabel("Access Password Action").selectOption("remove");
  await page.getByLabel("Slug", { exact: true }).fill("editor-fern-public");
  await expect(page.getByRole("link", { name: "Visit Page" })).toHaveAttribute("href", "/editor-fern-renamed");
  await save.click();
  await expect(confirmation).toContainText("/editor-fern-public");
  await expect(confirmation).toContainText("Anyone with the link");
  await confirmation.getByRole("button", { name: "Cancel" }).click();
  await expect(page.getByLabel("Slug", { exact: true })).toHaveValue("editor-fern-public");
  await save.click();
  await confirmation.getByRole("button", { name: "Save Changes" }).click();
  await expect(page.getByRole("status")).toHaveText("Changes saved.");
  await visitor.clearCookies();
  await readPage.goto("/editor-fern-public");
  await expect(
    readPage.getByRole("heading", { name: "Editor fern" }),
  ).toBeVisible();
  await visitor.close();
  expect(
    (await context.cookies()).find((c) => c.name === "pagelet_session")
      .httpOnly,
  ).toBeTruthy();
  expect(
    await page.evaluate(() => [
      document.cookie,
      localStorage.length,
      sessionStorage.length,
    ]),
  ).toEqual(["", 0, 0]);
});

test("preserves work on validation, stale versions and revoked sessions, then deletes permanently", async ({
  page,
  context,
}) => {
  await page.goto("/_/app/login");
  await page.getByLabel("Username").fill("admin");
  await page.getByLabel("Account password").fill("browser test password");
  await page.getByRole("button", { name: "Log In" }).click();
  await expect(page).toHaveURL(/\/_\/app\/pages$/);
  await page.getByRole("link", { name: "New Page", exact: true }).click();
  await page.getByLabel("Markdown").fill("# Original");
  await page.getByRole("button", { name: "Publish Page" }).click();
  await expect(page.getByRole("status")).toHaveText("Page published.");
  const id = page.url().match(/pages\/(\d+)/)[1];
  const endpoint = `/_/api/v1/pages/${id}`;
  const record = await (await page.request.get(endpoint)).json();
  await page.getByLabel("Markdown").fill("# Unsaved fern");
  await page.getByLabel("Slug", { exact: true }).fill("INVALID");
  await page.getByRole("button", { name: "Save Changes" }).click();
  await expect(page.getByRole("alert")).toContainText("Check your slug");
  await expect(page.getByLabel("Markdown")).toHaveValue("# Unsaved fern");
  await page.getByLabel("Slug", { exact: true }).fill(record.slug);
  await page.request.post("/_/api/v1/pages", {
    headers: { "x-pagelet-csrf": "1" },
    data: { slug: "occupied", markdown: "" },
  });
  await page.getByLabel("Slug", { exact: true }).fill("occupied");
  await page.getByRole("button", { name: "Save Changes" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Save Changes" }).click();
  await expect(page.getByRole("alert")).toContainText("slug is already in use");
  await expect(page.getByRole("button", { name: "Reload Server Version" })).toBeHidden();
  await expect(page.getByLabel("Markdown")).toHaveValue("# Unsaved fern");
  await page.getByLabel("Slug", { exact: true }).fill(record.slug);
  const changed = await page.request.patch(endpoint, {
    headers: { "x-pagelet-csrf": "1" },
    data: { version: record.version, markdown: "# Changed elsewhere" },
  });
  expect(changed.ok()).toBeTruthy();
  await page.getByRole("button", { name: "Save Changes" }).click();
  await expect(page.getByRole("alert")).toContainText("Conflict");
  await expect(page.getByLabel("Markdown")).toHaveValue("# Unsaved fern");
  expect((await (await page.request.get(endpoint)).json()).markdown).toBe(
    "# Changed elsewhere",
  );
  await page.getByRole("button", { name: "Delete Permanently" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Delete Permanently" }).click();
  await expect(page.getByRole("alert")).toContainText("Conflict");
  expect((await page.request.get(endpoint)).status()).toBe(200);
  await page.getByRole("button", { name: "Reload Server Version" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Reload Server Version" }).click();
  await expect(page.getByLabel("Markdown")).toHaveValue("# Changed elsewhere");
  await page.getByLabel("Markdown").fill("# Recovered after expiry");
  await page.request.post("/_/api/v1/logout", {
    headers: { "x-pagelet-csrf": "1" },
  });
  await page.getByRole("button", { name: "Save Changes" }).click();
  await expect(page.getByRole("alert")).toContainText("session has ended");
  await expect(page.getByLabel("Markdown")).toHaveValue(
    "# Recovered after expiry",
  );
  await page.getByLabel("Username").fill("admin");
  await page.getByLabel("Account password").fill("browser test password");
  await page.getByRole("button", { name: "Log In" }).click();
  await page.getByRole("button", { name: "Save Changes" }).click();
  await expect(page.getByRole("status")).toHaveText("Changes saved.");
  await page.getByRole("button", { name: "Delete Permanently" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Delete Permanently" }).click();
  await expect(page.getByRole("status")).toContainText(
    "Page permanently deleted",
  );
  expect((await page.request.get(endpoint)).status()).toBe(404);
  expect(
    await page.evaluate(() => [localStorage.length, sessionStorage.length]),
  ).toEqual([0, 0]);
});

test("explains forbidden and deleted pages without showing an editable document", async ({
  page,
  request,
}) => {
  const headers = { "x-pagelet-csrf": "1" };
  await request.post("/_/api/v1/login", {
    headers,
    data: { username: "admin", password: "browser test password" },
  });
  const user = await request.post("/_/api/v1/users", {
    headers,
    data: {
      username: "editor-user",
      password: "ordinary password",
      role: "user",
    },
  });
  expect(user.status()).toBe(201);
  const created = await request.post("/_/api/v1/pages", {
    headers,
    data: { markdown: "# Administrator page" },
  });
  const record = await created.json();
  await page.goto(`/_/app/pages/${record.id}/edit`);
  await page.getByLabel("Username").fill("editor-user");
  await page.getByLabel("Account password").fill("incorrect password");
  await page.getByRole("button", { name: "Log In" }).click();
  await expect(page.getByRole("alert")).toHaveText(
    "Incorrect username or account password.",
  );
  await page.getByLabel("Account password").fill("ordinary password");
  await page.getByRole("button", { name: "Log In" }).click();
  await expect(page).toHaveURL(/\/_\/app\/pages$/);
  await page.goto(`/_/app/pages/${record.id}/edit`);
  await expect(page.getByRole("alert")).toContainText("do not have permission");
  await expect(page.getByLabel("Markdown")).toBeHidden();
  await page.goto("/_/app/pages/999999/edit");
  await expect(page.getByRole("alert")).toContainText("Page not found");
  await expect(page.getByLabel("Markdown")).toBeHidden();
});
