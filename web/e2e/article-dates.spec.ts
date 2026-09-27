import { expect, test } from "@playwright/test";

test("publication and saved dates remain distinct and metadata completion does not move controls", async ({
  page,
}) => {
  let release!: () => void;
  const held = new Promise<void>((resolve) => (release = resolve));
  const savedAt = "2026-09-27T17:21:36Z";
  await page.route("**/api/bootstrap*", async (route) => {
    const data = await (await route.fetch()).json();
    data.articlePage.articles = data.articlePage.articles.map(
      (article: object) => ({
        ...article,
        savedAt,
        publishedAt: null,
        publicationStatus: "unknown",
        publicationSources: [],
      }),
    );
    await route.fulfill({ json: data });
  });
  await page.route("**/api/articles/1?*", async (route) => {
    const data = await (await route.fetch()).json();
    await held;
    await route.fulfill({
      json: {
        ...data,
        savedAt,
        publishedAt: "2020-02-03",
        publicationStatus: "known",
        publicationSources: ["json-ld:datePublished"],
      },
    });
  });
  await page.goto("/reader");
  const reader = page.getByRole("article", { name: "Article reader" });
  const dates = reader.locator(".article-dates");
  await expect(dates).toContainText("Published Unknown");
  await expect(dates).toContainText("Saved 2026-sep-27 17:21:36");
  const toolbar = reader.locator(".reader-toolbar");
  const source = reader.locator(".reader-meta");
  const before = await toolbar.boundingBox();
  const sourceBefore = await source.boundingBox();
  release();
  await expect(dates).toContainText("Published 2020-feb-03");
  await expect(dates).not.toContainText("00:00:00");
  await expect(dates).toContainText("Saved 2026-sep-27 17:21:36");
  expect(await toolbar.boundingBox()).toEqual(before);
  expect(await source.boundingBox()).toEqual(sourceBefore);
  await page.screenshot({ path: test.info().outputPath("article-dates.png") });
});
