import { expect, test } from "@playwright/test";
test("unified search keeps controls stable, previews results, and restores browser history", async ({
  page,
}) => {
  let release: () => void = () => {};
  let calls = 0;
  await page.route("**/api/search/limits", (r) =>
    r.fulfill({
      json: { query_bytes: 512, page_size: 25, excerpt_characters: 220 },
    }),
  );
  await page.route("**/api/search?*", async (r) => {
    calls++;
    await new Promise<void>((resolve) => {
      release = resolve;
    });
    await r.fulfill({
      json: {
        items: [
          {
            title: "Search result",
            context: "Data",
            excerpt: "Full article text match",
            updated_at: "2026-09-29",
            target: { kind: "news", workspace: "ws", article: "1" },
          },
        ],
        has_more: false,
      },
    });
  });
  await page.goto("/reader");
  await page
    .getByRole("button", { name: "Search news and wiki", exact: true })
    .click();
  await expect(page).toHaveURL(/\/search$/);
  const field = page.getByRole("searchbox");
  await field.fill("Rust");
  const submit = page
    .getByRole("button", { name: "Search", exact: true })
    .last();
  const before = await submit.boundingBox();
  await submit.click();
  await expect(submit).toHaveAttribute("aria-busy", "true");
  await expect(submit).toBeDisabled();
  expect(await submit.boundingBox()).toEqual(before);
  await expect.poll(() => calls).toBe(1);
  const next = page.getByRole("button", { name: "Next →" });
  const nextBefore = await next.boundingBox();
  release();
  await page.getByRole("button", { name: /Search result/ }).click();
  await expect(page.getByLabel("Search preview")).toContainText("Async Rust");
  expect(await next.boundingBox()).toEqual(nextBefore);
  expect(await submit.boundingBox()).toEqual(before);
  await page.goBack();
  await expect(page.getByLabel("Search preview")).toContainText(
    "Select a result",
  );
  await expect(field).toHaveValue("Rust");
  await page.screenshot({
    path: "/tmp/reader-search-page.png",
    fullPage: true,
  });
});
