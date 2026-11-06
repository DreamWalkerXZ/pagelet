import { test, expect } from "@playwright/test";

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
  await pageRow.getByLabel("New owner ID").fill(userId);
  await pageRow.getByRole("button", { name: "Transfer ownership" }).click();
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
  const row = page.getByRole("row").filter({ hasText: record.slug });
  await expect(page.locator("#rows tr")).toHaveCount(1);
  await page.request.patch(`/_/api/v1/pages/${record.id}`, {
    headers,
    data: { version: record.version, markdown: "# Updated elsewhere" },
  });
  await row.getByLabel("New owner ID").fill("1");
  await row.getByRole("button", { name: "Transfer ownership" }).click();
  await expect(page.getByRole("alert")).toContainText("Conflict");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await row.getByLabel("New owner ID").fill("1");
  await row.getByRole("button", { name: "Transfer ownership" }).click();
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
