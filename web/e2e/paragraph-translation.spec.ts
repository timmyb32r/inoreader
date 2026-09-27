import { wireFixture } from "./wire-fixtures.mjs";
import { expect, test } from "@playwright/test";
import type { ParagraphJob } from "../src/api/ai";

test("paragraph translation preserves article geometry and links, deduplicates clicks, and shows instant dictionary cards", async ({
  page,
}) => {
  const article = {
    id: "a",
    title: "Column storage",
    url: "https://example.test/a",
    source: "Example",
    excerpt: "Вводный текст",
    body: [],
    bodyHtml:
      '<h2>技术细节</h2><p>磁<strong>盘</strong> 读取。</p><p>下一段保持原位。</p><p><a href="https://example.test/source">来源</a></p>',
    fullText: "ready",
    age: "2026-09-27T10:00:00Z",
    read: false,
    later: false,
  };
  const result = {
    source: "磁盘 读取。",
    translation: "Чтение с диска.",
    segments: [
      {
        kind: "word" as const,
        source: "磁盘",
        pinyin: "cípán",
        translation: "диск",
      },
      { kind: "literal" as const, source: " " },
      {
        kind: "word" as const,
        source: "读取",
        pinyin: "dúqǔ",
        translation: "читать",
      },
      { kind: "literal" as const, source: "。" },
    ],
  };
  let jobs: ParagraphJob[] = [],
    posts = 0,
    done = false;
  const base = {
    id: "job",
    workspaceId: "ws",
    articleId: "a",
    source: result.source,
    model: "deepseek-flash",
  };
  await page.route("**/api/**", async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path === "/api/bootstrap")
      return route.fulfill({
        json: wireFixture({
          account: { id: "owner", displayName: "Author", initials: "AU" },
          workspaces: [{ id: "ws", name: "Personal", archived: false }],
          activeWorkspaceId: "ws",
          subscriptions: [],
          articlePage: { articles: [article], total: 1, unreadTotal: 1 },
        }),
      });
    if (path === "/api/articles/a" || path.endsWith("/state"))
      return route.fulfill({ json: wireFixture(article) });
    if (path === "/api/articles")
      return route.fulfill({
        json: wireFixture({ articles: [article], total: 1, unreadTotal: 1 }),
      });
    if (path === "/api/ai/profile")
      return route.fulfill({
        json: wireFixture({ configured: true, enabled: true }),
      });
    if (path.endsWith("/translations")) {
      if (route.request().method() === "POST") {
        posts++;
        expect(route.request().postDataJSON().source).toBe(result.source);
        jobs = [{ ...base, status: "generating" }];
        return route.fulfill({ json: wireFixture(jobs[0]) });
      }
      return route.fulfill({
        json: wireFixture(
          done ? [{ ...base, status: "completed", result }] : jobs,
        ),
      });
    }
    return route.fulfill({ json: wireFixture([]) });
  });
  await page.goto("/reader");
  const content = page.locator(".article-content"),
    paragraph = content.locator("p").first(),
    following = content.locator("p").nth(1),
    button = page.getByRole("button", {
      name: "Translate paragraphs",
      exact: true,
    });
  const before = await following.boundingBox(),
    toolbar = await page.locator(".reader-toolbar").boundingBox(),
    text = await content.textContent();
  await button.click();
  await expect(button).toHaveAttribute("aria-pressed", "true");
  await expect(paragraph).toHaveAttribute("data-translatable", "true");

  const title = page.locator(".reader-body h1"),
    intro = page.locator(".reader-deck"),
    heading = content.locator("h2");
  for (const block of [title, intro, heading]) {
    await expect(block).toHaveAttribute("data-translatable", "true");
    await block.hover();
    expect(
      await block.evaluate((el) => getComputedStyle(el).outlineColor),
    ).not.toBe("rgba(0, 0, 0, 0)");
  }
  expect(await following.boundingBox()).toEqual(before);
  expect(await page.locator(".reader-toolbar").boundingBox()).toEqual(toolbar);
  await paragraph.dblclick();
  await expect(
    page.getByRole("region", { name: "Paragraph translation" }),
  ).toHaveAttribute("aria-busy", "true");
  expect(posts).toBe(1);
  // Double-click can select browser text; annotations must wait until it clears.
  await page.evaluate(() => window.getSelection()?.removeAllRanges());
  done = true;
  await expect(content.locator('[data-translation-word="0"]')).toHaveCount(2);
  expect(await content.textContent()).toBe(text);
  expect(await following.boundingBox()).toEqual(before);
  expect(await page.locator(".reader-toolbar").boundingBox()).toEqual(toolbar);
  const span = content.locator('[data-translation-word="0"]').first();
  await span.hover();
  expect(await page.getByRole("tooltip").isVisible()).toBe(true);
  await expect(page.getByRole("tooltip")).toContainText("cípán");
  await expect(page.getByRole("tooltip")).toContainText("диск");
  await content.locator('[data-translation-word="1"]').hover();
  await expect(page.getByRole("tooltip")).toContainText("dúqǔ");
  expect(posts).toBe(1);
  await expect(content.getByRole("link", { name: "来源" })).toHaveAttribute(
    "href",
    "https://example.test/source",
  );
  await page
    .getByRole("button", { name: "Close paragraph translation" })
    .click();
  await expect(
    page.getByRole("region", { name: "Paragraph translation" }),
  ).toHaveCount(0);
  await content.locator('[data-translation-word="0"]').first().click();
  await expect(
    page.getByRole("region", { name: "Paragraph translation" }),
  ).toContainText("Чтение с диска.");
  expect(posts).toBe(1);
  expect(await following.boundingBox()).toEqual(before);
  expect(await page.locator(".reader-toolbar").boundingBox()).toEqual(toolbar);
  await page.screenshot({
    path: "/tmp/inoreader-paragraph-translation.png",
    fullPage: true,
  });
  await page.keyboard.press("Escape");
  await expect(page.getByRole("tooltip")).toHaveCount(0);
  await button.click();
  await expect(content.locator("[data-translation-word]")).toHaveCount(0);
  expect(await following.boundingBox()).toEqual(before);
});

for (const target of [
  "h1",
  ".reader-deck",
  ".article-content h2",
  ".article-content [data-translation-fragment]",
])
  test(`translates ${target} above body paragraphs without moving controls`, async ({
    page,
  }) => {
    const source = "磁盘",
      result = {
        source,
        translation: "Диск",
        segments: [
          { kind: "word", source, pinyin: "cípán", translation: "диск" },
        ],
      };
    const article = {
      id: "a",
      title: target === "h1" ? source : "Title",
      excerpt: target === ".reader-deck" ? source : "Introduction",
      source: "Example",
      url: "https://example.test/a",
      body: [],
      bodyHtml: `${target.includes("fragment") ? source : ""}<h2>${target === ".article-content h2" ? source : "Heading"}</h2><p>Body paragraph stays here.</p>`,
      fullText: "ready",
      age: "2026-09-27",
      read: false,
      later: false,
    };
    let posts = 0;
    await page.route("**/api/**", (route) => {
      const path = new URL(route.request().url()).pathname;
      if (path === "/api/bootstrap")
        return route.fulfill({
          json: wireFixture({
            account: { id: "owner", displayName: "Author", initials: "AU" },
            workspaces: [{ id: "ws", name: "Personal", archived: false }],
            activeWorkspaceId: "ws",
            subscriptions: [],
            articlePage: { articles: [article], total: 1, unreadTotal: 1 },
          }),
        });
      if (path === "/api/articles/a" || path.endsWith("/state"))
        return route.fulfill({ json: wireFixture(article) });
      if (path === "/api/articles")
        return route.fulfill({
          json: wireFixture({ articles: [article], total: 1, unreadTotal: 1 }),
        });
      if (path === "/api/ai/profile")
        return route.fulfill({
          json: wireFixture({ configured: true, enabled: true }),
        });
      if (
        path.endsWith("/translations") &&
        route.request().method() === "POST"
      ) {
        posts++;
        expect(route.request().postDataJSON().source).toBe(source);
        return route.fulfill({
          json: wireFixture({
            id: "job",
            workspaceId: "ws",
            articleId: "a",
            source,
            model: "deepseek-flash",
            status: "completed",
            result,
          }),
        });
      }
      return route.fulfill({ json: wireFixture([]) });
    });
    await page.goto("/reader");
    const block = page.locator(".reader-body").locator(target),
      following = page.locator(".article-content p"),
      toolbar = page.locator(".reader-toolbar");
    const before = await following.boundingBox(),
      toolbarBefore = await toolbar.boundingBox();
    await page
      .getByRole("button", { name: "Translate paragraphs", exact: true })
      .click();
    await expect(block).toHaveAttribute("data-translatable", "true");
    await block.click();
    await expect(
      page.getByRole("region", { name: "Paragraph translation" }),
    ).toContainText("Диск");
    await block.locator("[data-translation-word]").hover();
    await expect(page.getByRole("tooltip")).toContainText("cípán");
    expect(posts).toBe(1);
    expect(await following.boundingBox()).toEqual(before);
    expect(await toolbar.boundingBox()).toEqual(toolbarBefore);
  });
