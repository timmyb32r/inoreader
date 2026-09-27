import { wireFixture } from "./wire-fixtures.mjs";
import { expect, test } from "@playwright/test";

test("Feed contains unread articles; reading keeps rows fixed and subscription history remains available", async ({
  page,
}) => {
  const articles = [
    { id: "1", title: "Unread story", read: false },
    { id: "2", title: "Another unread story", read: false },
    { id: "3", title: "An older read story", read: true },
  ].map((value) => ({
    ...value,
    url: "https://example.test/story",
    source: "Example",
    subscriptionIds: ["sub"],
    excerpt: "Excerpt",
    body: [],
    savedAt: "2026-09-26T00:00:00Z",
    later: false,
    fullText: "ready",
  }));
  const articlePage = (view = "feed") => {
    const visible = articles.filter(
      (article) =>
        view === "subscription" ||
        (view === "later" ? article.later : !article.read),
    );
    return {
      articles: visible,
      total: visible.length,
      unreadTotal: articles.filter((article) => !article.read).length,
    };
  };
  await page.route("**/api/**", async (route) => {
    const url = new URL(route.request().url());
    if (url.pathname === "/api/bootstrap")
      return route.fulfill({
        json: wireFixture({
          account: { id: "fixture", displayName: "Test", initials: "T" },
          workspaces: [{ id: "ws", name: "Personal", archived: false }],
          activeWorkspaceId: "ws",
          subscriptions: [
            {
              id: "sub",
              name: "Example",
              count: 3,
              unreadCount: 2,
              status: "active",
            },
          ],
          articlePage: articlePage(),
        }),
      });
    if (url.pathname === "/api/articles")
      return route.fulfill({
        json: wireFixture(articlePage(url.searchParams.get("view") ?? "feed")),
      });
    const article = articles.find(
      (value) =>
        url.pathname === `/api/articles/${value.id}` ||
        url.pathname === `/api/articles/${value.id}/state`,
    );
    if (article) {
      if (route.request().method() === "POST")
        Object.assign(article, route.request().postDataJSON());
      return route.fulfill({ json: wireFixture(article) });
    }
    return route.continue();
  });
  await page.goto("/reader");
  await expect(
    page.getByRole("button", { name: "Feed (2)", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "An older read story" }),
  ).toHaveCount(0);
  const nav = page.getByRole("navigation", { name: "Library" });
  for (const name of ["Unread", "Saved", "Trash", "All"])
    await expect(nav.getByRole("button", { name, exact: true })).toHaveCount(0);
  const row = page.getByRole("heading", {
    name: "Unread story",
    exact: true,
    level: 2,
  });
  const before = await row.boundingBox();
  await row.click();
  await expect(
    page.getByRole("button", { name: "Feed (1)", exact: true }),
  ).toBeVisible();
  expect(await row.boundingBox()).toEqual(before);
  await expect(
    page.getByRole("heading", { name: "Unread story", exact: true, level: 1 }),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: "Move to trash" })).toHaveCount(
    0,
  );
  await page.getByRole("button", { name: "Feed (1)", exact: true }).click();
  await expect(row).toHaveCount(0);
  await page
    .getByRole("navigation", { name: "Subscriptions", exact: true })
    .getByRole("button", { name: /Example/ })
    .click();
  await expect(
    page.getByRole("heading", { name: "An older read story", level: 2 }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Unread story", exact: true, level: 2 }),
  ).toBeVisible();
});
