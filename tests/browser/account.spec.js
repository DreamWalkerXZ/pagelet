import { test, expect } from "@playwright/test";

test("personal lists isolate owners, search and paginate; credentials transition without browser storage", async ({
  page,
  request,
  browser,
}) => {
  const headers = { "x-pagelet-csrf": "1" };
  await request.post("/_/api/v1/login", {
    headers,
    data: { username: "admin", password: "browser test password" },
  });
  await request.post("/_/api/v1/users", {
    headers,
    data: {
      username: "account-owner",
      password: "original password",
      role: "user",
    },
  });
  await request.post("/_/api/v1/pages", {
    headers,
    data: { slug: "another-owner", markdown: "private management" },
  });
  await page.goto("/_/app/login");
  await page.getByLabel("Username").fill("account-owner");
  await page.getByLabel("Account password").fill("original password");
  await page.getByRole("button", { name: "Log in", exact: true }).click();
  await expect(page).toHaveURL(/\/_\/app\/pages$/);
  for (let i = 0; i < 21; i++) {
    expect(
      (
        await page.request.post("/_/api/v1/pages", {
          headers,
          data: {
            slug: `personal-${String(i).padStart(2, "0")}`,
            markdown: "",
          },
        })
      ).status(),
    ).toBe(201);
  }
  const otherSession = await browser.newContext();
  const accountURL = new URL("/_/api/v1/me", page.url()).href;
  await otherSession.request.post(new URL("/_/api/v1/login", page.url()).href, {
    headers,
    data: { username: "account-owner", password: "original password" },
  });
  await page.goto("/_/app/pages");
  await expect(
    page.getByRole("link", { name: "personal-20", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("link", { name: "another-owner", exact: true }),
  ).toHaveCount(0);
  await page.getByRole("button", { name: "Next", exact: true }).click();
  await expect(
    page.getByRole("link", { name: "personal-00", exact: true }),
  ).toBeVisible();
  await page.getByLabel("Search slugs").fill("personal-14");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect(
    page.getByRole("link", { name: "personal-14", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Next", exact: true }),
  ).toBeDisabled();
  await page.getByRole("link", { name: "Account", exact: true }).click();
  await page
    .getByRole("button", { name: "Reset API token", exact: true })
    .click();
  await expect(page.getByLabel("New API token", { exact: true })).toBeVisible();
  const token = await page
    .getByLabel("New API token", { exact: true })
    .inputValue();
  expect(token.length).toBeGreaterThan(20);
  expect((await page.request.get("/_/api/v1/me")).status()).toBe(200);
  await page.reload();
  await expect(page.getByLabel("New API token", { exact: true })).toBeHidden();
  await page.getByLabel("Current password").fill("original password");
  await page
    .getByLabel("New password", { exact: true })
    .fill("replacement password");
  await page
    .getByRole("button", { name: "Change password", exact: true })
    .click();
  await expect(page.getByLabel("New API token", { exact: true })).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Dismiss token", exact: true }),
  ).toBeVisible();
  expect((await page.request.get("/_/api/v1/me")).status()).toBe(401);
  expect((await otherSession.request.get(accountURL)).status()).toBe(401);
  await otherSession.close();
  const outsider = await browser.newContext();
  expect(
    (
      await outsider.request.get(new URL("/_/api/v1/me", page.url()).href, {
        headers: { authorization: `Bearer ${token}` },
      })
    ).status(),
  ).toBe(401);
  await outsider.close();
  expect(
    await page.evaluate(() => [localStorage.length, sessionStorage.length]),
  ).toEqual([0, 0]);
  await page.getByRole("button", { name: "Dismiss token", exact: true }).click();
  await page.getByLabel("Username").fill("account-owner");
  await page.getByLabel("Account password").fill("replacement password");
  await page.getByRole("button", { name: "Log in", exact: true }).click();
  await expect(page).toHaveURL(/\/_\/app\/pages$/);
  await page.goto("/_/app/account");
  await page.getByRole("button", { name: "Log out", exact: true }).click();
  await expect(page).toHaveURL(/\/_\/app\/login$/);
  expect((await page.request.get("/_/api/v1/me")).status()).toBe(401);
});

test("administrators see only their own pages in My pages", async ({
  page,
  request,
}) => {
  const headers = { "x-pagelet-csrf": "1" };
  await request.post("/_/api/v1/login", {
    headers,
    data: { username: "admin", password: "browser test password" },
  });
  const created = await request.post("/_/api/v1/users", {
    headers,
    data: {
      username: "isolated-owner",
      password: "isolated password",
      role: "user",
    },
  });
  const { token } = await created.json();
  await request.post("/_/api/v1/pages", {
    headers: { authorization: `Bearer ${token}` },
    data: { slug: "other-private-page", markdown: "" },
  });
  await request.post("/_/api/v1/pages", {
    headers,
    data: { slug: "admin-personal-page", markdown: "" },
  });
  await page.goto("/_/app/login");
  await page.getByLabel("Username").fill("admin");
  await page.getByLabel("Account password").fill("browser test password");
  await page.getByRole("button", { name: "Log in", exact: true }).click();
  await expect(page).toHaveURL(/\/_\/app\/pages$/);
  await page.goto("/_/app/pages");
  await expect(
    page.getByRole("link", { name: "admin-personal-page", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("link", { name: "other-private-page", exact: true }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("link", { name: "All pages", exact: true }),
  ).toBeVisible();
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(
    page.getByRole("button", { name: "Search", exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBeTruthy();
  await expect(page.locator("#rows td:last-child").first()).toContainText(
    String(new Date().getFullYear()),
  );
  await page.screenshot({
    path: "output/playwright/account-list-mobile.png",
    fullPage: true,
  });
});
