import { expect, test } from "@playwright/test";

test("untitled post keeps its row and adjacent controls stable on selection", async ({
  page,
}) => {
  await page.route("**/api/bootstrap*", async (route) => {
    const data = await (await route.fetch()).json();
    data.articlePage.articles[0].title = "";
    data.articlePage.articles[0].excerpt = "Original Telegram caption";
    await route.fulfill({ json: data });
  });
  await page.route("**/api/articles/1?*", async (route) => {
    const data = await (await route.fetch()).json();
    await route.fulfill({
      json: { ...data, title: "", excerpt: "Original Telegram caption" },
    });
  });
  await page.goto("/reader");
  const row = page.locator(".article-row").first();
  const label = row.getByLabel("Post has no title");
  await expect(label).toBeVisible();
  const later = row.locator(".row-later");
  const before = await later.boundingBox();
  await label.click();
  await expect(row).toHaveClass(/selected/);
  await expect(row).toContainText("Original Telegram caption");
  expect(await later.boundingBox()).toEqual(before);
});
