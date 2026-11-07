import { test, expect } from "@playwright/test";

test("created user's one-time token survives a failed list refresh", async ({
  page,
}) => {
  const headers = { "x-pagelet-csrf": "1" };
  await page.request.post("/_/api/v1/login", {
    headers,
    data: { username: "admin", password: "browser test password" },
  });
  await page.goto("/_/app/admin/users");
  await expect(page.locator("#rows tr")).not.toHaveCount(0);
  await page.route("**/_/api/v1/users?*", (route) =>
    route.fulfill({ status: 503 }),
  );
  let creations = 0;
  page.on("request", (request) => {
    if (request.method() === "POST" && request.url().endsWith("/_/api/v1/users"))
      creations++;
  });
  await page.getByLabel("New username").fill("refresh-failure-account");
  await page.getByLabel("Initial password").fill("ordinary password");
  await page.getByRole("button", { name: "Create user" }).click();
  await expect(page.getByRole("dialog")).toContainText("User created");
  await expect(page.getByLabel("New API token")).toBeVisible();
  const token = await page.getByLabel("New API token").inputValue();
  expect(
    (
      await page.request.get("/_/api/v1/me", {
        headers: { authorization: `Bearer ${token}` },
      })
    ).status(),
  ).toBe(200);
  await page.getByRole("button", { name: "Dismiss token" }).click();
  await expect(page.getByRole("alert")).toContainText("User list refresh failed");
  await page.unroute("**/_/api/v1/users?*");
  await page.getByLabel("Search", { exact: true }).fill("refresh-failure-account");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect(page.locator("#rows tr")).toHaveCount(1);
  await expect(page.getByLabel("New API token")).toBeHidden();
  expect(creations).toBe(1);
  await page.reload();
  await expect(page.getByLabel("New API token")).toBeHidden();
});

test("administrator preserves Unicode passwords when creating and resetting users", async ({
  page,
  browser,
}) => {
  const headers = { "x-pagelet-csrf": "1" };
  await page.request.post("/_/api/v1/login", {
    headers,
    data: { username: "admin", password: "browser test password" },
  });
  await page.goto("/_/app/admin/users");
  await page.getByLabel("New username").fill("unicode-account");
  const initial = "😀".repeat(128);
  await page.getByLabel("Initial password").fill(initial);
  await page.getByRole("button", { name: "Create user" }).click();
  await expect(page.getByLabel("New API token")).toBeVisible();
  const ordinary = await browser.newContext();
  const login = (password) =>
    ordinary.request.post("/_/api/v1/login", {
      headers,
      data: { username: "unicode-account", password },
    });
  expect((await login(initial)).status()).toBe(200);
  await page.getByRole("button", { name: "Dismiss token" }).click();
  const replacement = "🦊".repeat(100);
  const row = page.getByRole("row").filter({ hasText: "unicode-account" });
  await row.getByLabel("New account password").fill(replacement);
  page.once("dialog", (dialog) => dialog.accept());
  await row.getByRole("button", { name: "Reset password" }).click();
  await expect(page.getByRole("dialog")).toContainText("Password reset");
  expect((await login(replacement)).status()).toBe(200);
  await ordinary.close();
});

test("administrator creates users, transfers protected pages and resets credentials once", async ({
  page,
}) => {
  const headers = { "x-pagelet-csrf": "1" };
  await page.request.post("/_/api/v1/login", {
    headers,
    data: { username: "admin", password: "browser test password" },
  });
  await page.goto("/_/app/admin/users");
  await page.getByLabel("New username").fill("admin-workflow");
  await page.getByLabel("Initial password").fill("ordinary password");
  await page.getByRole("button", { name: "Create user" }).click();
  await expect(page.getByLabel("New API token")).toBeVisible();
  const originalToken = await page.getByLabel("New API token").inputValue();
  const row = page.getByRole("row").filter({ hasText: "admin-workflow" });
  const { users } = await (
    await page.request.get("/_/api/v1/users?search=admin-workflow")
  ).json();
  const userId = String(users[0].id);
  await page.getByRole("button", { name: "Dismiss token" }).click();
  await expect(page.getByLabel("New API token")).toBeHidden();
  const created = await page.request.post("/_/api/v1/pages", {
    headers,
    data: {
      slug: "admin-workflow-page",
      markdown: "# Managed",
      access_password: "secret",
    },
  });
  const record = await created.json();
  await page.goto("/_/app/admin/pages");
  const pageRow = page.getByRole("row").filter({ hasText: record.slug });
  await expect(pageRow.getByRole("link", { name: record.slug, exact: true })).toHaveAttribute("href", "/" + record.slug);
  await pageRow.getByRole("link", { name: /Edit/ }).click();
  await page.getByLabel("New owner ID").fill(userId);
  page.once("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Transfer ownership" }).click();
  await expect(page.getByRole("status")).toContainText("Ownership transferred");
  await page.goto("/_/app/admin/users");
  page.once("dialog", (dialog) => dialog.accept());
  await row.getByRole("button", { name: "Delete user" }).click();
  await expect(page.getByRole("alert")).toContainText("Transfer or delete");
  await row.getByRole("button", { name: "Reset API token" }).click();
  await expect(page.getByLabel("New API token")).toBeVisible();
  expect(
    (
      await page.request.get("/_/api/v1/me", {
        headers: { authorization: `Bearer ${originalToken}` },
      })
    ).status(),
  ).toBe(401);
  await page.reload();
  await expect(page.getByLabel("New API token")).toBeHidden();
  const current = await (
    await page.request.get(`/_/api/v1/pages/${record.id}`)
  ).json();
  await page.request.delete(`/_/api/v1/pages/${record.id}`, {
    headers,
    data: { version: current.version },
  });
  page.once("dialog", (dialog) => dialog.accept());
  await row.getByRole("button", { name: "Delete user" }).click();
  await expect(row).toHaveCount(0);
});

test("transfer rejects stale versions, revokes old access and ordinary users cannot administer", async ({
  page,
  browser,
}) => {
  const headers = { "x-pagelet-csrf": "1" };
  await page.request.post("/_/api/v1/login", {
    headers,
    data: { username: "admin", password: "browser test password" },
  });
  const createdUser = await page.request.post("/_/api/v1/users", {
    headers,
    data: {
      username: "transfer-old-owner",
      password: "ordinary password",
      role: "user",
    },
  });
  const { user, token } = await createdUser.json();
  const created = await page.request.post("/_/api/v1/pages", {
    headers: { authorization: `Bearer ${token}` },
    data: {
      slug: "transfer-revocation",
      markdown: "# Transfer protected",
      access_password: "protected",
    },
  });
  const record = await created.json();
  const visitor = await browser.newContext();
  await visitor.request.post(
    "/_/api/v1/pages/by-slug/transfer-revocation/verify-password",
    { headers, data: { password: "protected" } },
  );
  expect(
    (
      await visitor.request.get("/_/api/v1/pages/by-slug/transfer-revocation")
    ).status(),
  ).toBe(200);
  await page.goto("/_/app/admin/pages");
  await page.getByLabel("Search", { exact: true }).fill(record.slug);
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await page.getByRole("link", { name: "Edit " + record.slug }).click();
  await expect(page.getByLabel("Markdown")).toHaveValue("# Transfer protected");
  await page.request.patch(`/_/api/v1/pages/${record.id}`, {
    headers,
    data: { version: record.version, markdown: "# Updated elsewhere" },
  });
  await page.getByLabel("New owner ID").fill("1");
  page.once("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Transfer ownership" }).click();
  await expect(page.getByRole("alert")).toContainText("Conflict");
  page.once("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Reload server version" }).click();
  await expect(page.getByLabel("Markdown")).toHaveValue("# Updated elsewhere");
  await page.getByLabel("Markdown").fill("# Unsaved after transfer");
  await page.getByLabel("New owner ID").fill("1");
  page.once("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Transfer ownership" }).click();
  await expect(page.getByLabel("Markdown")).toHaveValue("# Unsaved after transfer");
  await expect(page.getByRole("status")).toContainText("Ownership transferred");
  expect(
    (
      await visitor.request.get("/_/api/v1/pages/by-slug/transfer-revocation")
    ).status(),
  ).toBe(401);
  expect(
    (
      await page.request.get(`/_/api/v1/pages/${record.id}`, {
        headers: { authorization: `Bearer ${token}` },
      })
    ).status(),
  ).toBe(403);
  expect(
    (
      await page.request.get("/_/api/v1/pages/by-slug/transfer-revocation", {
        headers: { authorization: `Bearer ${token}` },
      })
    ).status(),
  ).toBe(401);
  const ordinary = await browser.newContext();
  await ordinary.request.post("/_/api/v1/login", {
    headers,
    data: { username: user.username, password: "ordinary password" },
  });
  const ordinaryPage = await ordinary.newPage();
  await ordinaryPage.goto("/_/app/admin/users");
  await expect(ordinaryPage.getByRole("alert")).toContainText(
    "Administrator access is required",
  );
  await expect(
    ordinaryPage.getByRole("button", { name: "Create user" }),
  ).toBeHidden();
  for (const path of [
    "/users",
    "/users/1/token",
    `/pages/${record.id}/owner`,
  ]) {
    const response = await ordinary.request.post(`/_/api/v1${path}`, {
      headers,
      data: { version: 3, owner_id: user.id },
    });
    expect(response.status()).toBe(403);
  }
  await ordinary.close();
  await visitor.close();
});

test("password reset rotates token and revokes sessions; final administrator cannot be deleted", async ({
  page,
  browser,
}) => {
  const headers = { "x-pagelet-csrf": "1" };
  await page.request.post("/_/api/v1/login", {
    headers,
    data: { username: "admin", password: "browser test password" },
  });
  const created = await page.request.post("/_/api/v1/users", {
    headers,
    data: {
      username: "reset-account",
      password: "ordinary password",
      role: "user",
    },
  });
  const { user, token } = await created.json();
  const ordinary = await browser.newContext();
  await ordinary.request.post("/_/api/v1/login", {
    headers,
    data: { username: user.username, password: "ordinary password" },
  });
  await page.goto("/_/app/admin/users");
  const row = page.getByRole("row").filter({ hasText: user.username });
  await row.getByLabel("New account password").fill("replacement password");
  page.once("dialog", (dialog) => dialog.accept());
  await row.getByRole("button", { name: "Reset password" }).click();
  await expect(page.getByLabel("New API token")).toBeVisible();
  const replacement = await page.getByLabel("New API token").inputValue();
  expect((await ordinary.request.get("/_/api/v1/me")).status()).toBe(401);
  expect(
    (
      await page.request.get("/_/api/v1/me", {
        headers: { authorization: `Bearer ${token}` },
      })
    ).status(),
  ).toBe(401);
  expect(
    (
      await page.request.get("/_/api/v1/me", {
        headers: { authorization: `Bearer ${replacement}` },
      })
    ).status(),
  ).toBe(200);
  await page.reload();
  await expect(page.getByLabel("New API token")).toBeHidden();
  const admin = page
    .getByRole("row")
    .filter({ has: page.getByRole("cell", { name: "admin", exact: true }) });
  page.once("dialog", (dialog) => dialog.accept());
  await admin.getByRole("button", { name: "Delete user" }).click();
  await expect(page.getByRole("alert")).toContainText("final administrator");
  expect(
    await page.evaluate(() => [
      localStorage.length,
      sessionStorage.length,
      document.cookie,
    ]),
  ).toEqual([0, 0, ""]);
  await ordinary.close();
});

test("management lists paginate metadata, search names and filter page owners", async ({
  page,
}) => {
  const headers = { "x-pagelet-csrf": "1" };
  await page.request.post("/_/api/v1/login", {
    headers,
    data: { username: "admin", password: "browser test password" },
  });
  const response = await page.request.post("/_/api/v1/users", {
    headers,
    data: {
      username: "list-owner",
      password: "ordinary password",
      role: "user",
    },
  });
  const { user, token } = await response.json();
  for (let index = 0; index < 21; index++) {
    const response = await page.request.post("/_/api/v1/pages", {
      headers: { authorization: `Bearer ${token}` },
      data: { slug: `list-page-${index}`, markdown: "" },
    });
    expect(response.status()).toBe(201);
  }
  await page.goto("/_/app/admin/pages");
  await page.getByLabel("Owner ID", { exact: true }).fill(String(user.id));
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect(page.locator("#count")).toHaveText("21 pages");
  await expect(page.locator("#rows tr")).toHaveCount(20);
  await page.getByRole("button", { name: "Next", exact: true }).click();
  await expect(page.locator("#page-number")).toHaveText("Page 2");
  await expect(page.locator("#rows tr")).toHaveCount(1);
  await expect(
    page.getByRole("button", { name: "Next", exact: true }),
  ).toBeDisabled();
  await page.getByLabel("Search", { exact: true }).fill("list-page-20");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect(page.locator("#count")).toHaveText("1 pages");
  await expect(page.locator("#page-number")).toHaveText("Page 1");
  await page.goto("/_/app/admin/users");
  await page.getByLabel("Search", { exact: true }).fill("list-owner");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect(page.locator("#count")).toHaveText("1 users");
  await expect(
    page.getByRole("row").filter({ hasText: "list-owner" }),
  ).toContainText("user");
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({
    path: "output/playwright/admin-users-mobile.png",
    fullPage: true,
  });
});
