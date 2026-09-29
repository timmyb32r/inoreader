import { expect, test } from "@playwright/test";
import { wireFixture } from "./wire-fixtures.mjs";

test("article URLs restore read articles outside the current Feed through reload and history", async ({
  page,
}) => {
  const articles = [1, 2].map((id) => ({
    id: String(id),
    title: `Article ${id}`,
    url: `https://example.test/${id}`,
    source: "Example",
    excerpt: "Intro",
    body: ["Text"],
    fullText: "ready",
    savedAt: "2026-09-29T10:00:00Z",
    read: false,
    later: false,
  }));
  const batch = () => ({
    articles: articles.filter((a) => !a.read),
    total: articles.filter((a) => !a.read).length,
    unreadTotal: articles.filter((a) => !a.read).length,
  });
  await page.route("**/api/**", async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path === "/api/bootstrap")
      return route.fulfill({
        json: wireFixture({
          account: { id: "owner", displayName: "Owner", initials: "O" },
          workspaces: [{ id: "ws", name: "Personal", archived: false }],
          activeWorkspaceId: "ws",
          subscriptions: [],
          articlePage: batch(),
        }),
      });
    if (path === "/api/articles")
      return route.fulfill({ json: wireFixture(batch()) });
    const match = /^\/api\/articles\/(1|2)(\/state)?$/.exec(path);
    if (match) {
      const article = articles[Number(match[1]) - 1];
      if (match[2]) Object.assign(article, route.request().postDataJSON());
      return route.fulfill({ json: wireFixture(article) });
    }
    return route.continue();
  });
  await page.goto("/reader");
  await page
    .locator(".article-row__main")
    .filter({ hasText: "Article 1" })
    .click();
  await expect(page).toHaveURL(/article=1/);
  await page
    .locator(".article-row__main")
    .filter({ hasText: "Article 2" })
    .click();
  await expect(page).toHaveURL(/article=2/);
  const heading = page.locator(".reader-body h1");
  await expect(heading).toHaveText("Article 2");
  await page.goBack();
  await expect(page).toHaveURL(/article=1/);
  await expect(heading).toHaveText("Article 1");
  await page.goForward();
  await expect(heading).toHaveText("Article 2");
  await page.reload();
  await expect(heading).toHaveText("Article 2");
  await expect(page.locator(".article-row__main")).toHaveCount(0);
  await page.getByRole("button", { name: "Mark unread", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Mark read", exact: true }),
  ).toBeVisible();
  await page.reload();
  await expect(heading).toHaveText("Article 2");
  await expect(page.locator(".article-row__main")).toHaveCount(1);
});

test("article wiki shortcut appears without shifting metadata and preserves the article return URL", async ({
  page,
}) => {
  let release: () => void = () => {};
  const pending = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/api/subscriptions/sub/wiki", async (route) => {
    await pending;
    await route.fulfill({
      json: {
        linked: true,
        namespace: "ns",
        page: {
          id: "p",
          name: "Source wiki",
          excerpt: "",
          excerpt_truncated: false,
          updated_at: "2026-09-29T00:00:00Z",
        },
      },
    });
  });
  await page.goto("/reader?article=1");
  const meta = page.locator(".reader-meta");
  await expect(meta).toBeVisible();
  const before = await meta.boundingBox();
  release();
  const link = page.getByRole("link", { name: "Open wiki page Source wiki" });
  await expect(link).toBeVisible();
  expect(await meta.boundingBox()).toEqual(before);
  await expect(link).toHaveAttribute(
    "href",
    /\/wiki\/ns\/page\/p\?return=.*article%3D1/,
  );
  await link.click();
  await expect(page).toHaveURL(/\/wiki\/ns\/page\/p/);
});

test("unlinked article offers creation without moving metadata or pending controls", async ({
  page,
}) => {
  await page.route("**/api/subscriptions/sub/wiki", (route) =>
    route.fulfill({ json: { linked: false, namespace: null, page: null } }),
  );
  await page.route("**/api/wiki/namespaces?*", (route) =>
    route.fulfill({
      json: {
        items: [{ id: "ns", name: "Private", role: "owner" }],
        has_more: false,
      },
    }),
  );
  await page.route("**/api/wiki/limits", (route) =>
    route.fulfill({
      json: {
        name_bytes: 512,
        markdown_bytes: 10000,
        search_bytes: 512,
        search_excerpt_characters: 200,
        page_size: 50,
        draft_save_delay_ms: 1000,
      },
    }),
  );
  await page.route("**/api/wiki/ns/subscription-root", (r) =>
    r.fulfill({
      json: {
        namespace: "ns",
        id: "root",
        revision: "r",
        name: "Subscriptions",
        markdown: "",
        deleted: false,
        author: "a",
        updated_at: "2026-09-29T00:00:00Z",
        parent: null,
      },
    }),
  );
  let release!: () => void;
  const pending = new Promise<void>((r) => {
    release = r;
  });
  let requests = 0;
  await page.route("**/api/wiki/ns/pages", async (route) => {
    requests++;
    await pending;
    await route.fulfill({
      status: 503,
      json: { error: "Temporarily unavailable" },
    });
  });
  await page.goto("/reader?article=1");
  const button = page.getByRole("button", {
    name: "Create and link wiki page",
    exact: true,
  });
  await expect(button).toBeVisible();
  const meta = page.locator(".reader-meta"),
    before = await meta.boundingBox();
  await button.click();
  const create = page.getByRole("button", {
    name: "Create and link",
    exact: true,
  });
  await expect(create).toBeEnabled();
  const box = await create.boundingBox();
  await create.click();
  await expect(create).toHaveAttribute("aria-busy", "true");
  await expect(create).toBeDisabled();
  expect(await create.boundingBox()).toEqual(box);
  expect(await meta.boundingBox()).toEqual(before);
  release();
  await expect(create).toBeEnabled();
  expect(await create.boundingBox()).toEqual(box);
  expect(requests).toBe(1);
});

test("opening subscriptions focuses search immediately and preserves control positions", async ({
  page,
}) => {
  await page.goto("/reader");
  const open = page.getByRole("button", { name: /^Subscriptions / });
  await open.click();
  const search = page.getByRole("searchbox", { name: "Search subscriptions" });
  await expect(search).toBeFocused();
  const close = page.getByRole("button", { name: "Close subscriptions" });
  const bounds = await close.boundingBox();
  await page.keyboard.type("Rust");
  await expect(search).toHaveValue("Rust");
  expect(await close.boundingBox()).toEqual(bounds);
  await close.click();
  await open.click();
  await expect(
    page.getByRole("searchbox", { name: "Search subscriptions" }),
  ).toBeFocused();
});
