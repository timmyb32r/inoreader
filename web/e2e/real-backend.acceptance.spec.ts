import { expect, test } from "@playwright/test";

test("real Rust API commits browser mutations and isolates account data", async ({
  page,
  context,
  baseURL,
}) => {
  const token = process.env.READER_ACCEPTANCE_TOKEN;
  if (!token || !baseURL)
    throw new Error("Hermetic acceptance fixture must provide session and URL");
  await context.addCookies([
    {
      name: "reader_session",
      value: token,
      url: baseURL,
      httpOnly: true,
      sameSite: "Strict",
    },
  ]);
  const privateResponse = await context.request.get(
    `/api/articles/${process.env.READER_PRIVATE_ARTICLE}?workspace_id=${process.env.READER_PRIVATE_WORKSPACE}`,
  );
  expect(privateResponse.status()).toBe(404);
  await page.goto("/reader");
  await expect(
    page.getByRole("heading", { name: /^Feed \(\d+\)$/ }),
  ).toBeVisible();
  await expect(page.locator(".article-row").first()).toContainText(
    "Persistent browser article",
  );
  const later = page
    .locator(".reader-toolbar")
    .getByRole("button", { name: "Read later", exact: true });
  await later.click();
  await expect(
    page.getByRole("button", { name: "Remove from later", exact: true }),
  ).toBeVisible();
  await page.reload();
  await expect(
    page.getByRole("button", { name: "Remove from later", exact: true }),
  ).toBeVisible();
  const bootstrap = await context.request.get("/api/bootstrap");
  expect(bootstrap.headers()["x-request-id"]).toMatch(/^[0-9a-f-]{36}$/);
  expect((await bootstrap.json()).articlePage.articles[0].later).toBe(true);
});
