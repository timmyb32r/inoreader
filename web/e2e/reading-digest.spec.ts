import { expect, test, type Page } from "@playwright/test";
import { wireFixture } from "./wire-fixtures.mjs";

async function fixture(page: Page) {
  const articles = Array.from({ length: 7 }, (_, i) => ({
    id: String(i + 1),
    title: `Digest article ${i + 1}`,
    source: i % 2 ? "Source B" : "Source A",
    subscriptionIds: [i % 2 ? "b" : "a", "secondary"],
    url: `https://example.test/${i + 1}`,
    body: ["Full original text."],
    excerpt: "Source excerpt, not an AI summary.",
    fullText: i === 2 ? "pending" : "ready",
    read: false,
    later: false,
    savedAt: "2026-09-30T06:00:00Z",
  }));
  const states = new Map(
    articles.map((a) => [
      a.id,
      {
        revision: "0",
        read: false,
        rating: null as number | null,
        ratedAt: null as string | null,
        reason: null as string | null,
      },
    ]),
  );
  const commands: any[] = [];
  const reads: string[] = [];
  let paid = 0,
    hold = false,
    lose = false;
  let release!: () => void;
  await page.route("**/api/**", async (route) => {
    const req = route.request(),
      url = new URL(req.url()),
      path = url.pathname;
    if (path === "/api/bootstrap")
      return route.fulfill({
        json: wireFixture({
          account: { id: "owner", displayName: "Owner", initials: "O" },
          workspaces: [{ id: "ws", name: "Data", archived: false }],
          activeWorkspaceId: "ws",
          subscriptions: [
            {
              id: "empty",
              name: "Empty source",
              count: 10,
              unreadCount: 0,
              status: "active",
            },
            {
              id: "a",
              name: "Source A",
              count: 4,
              unreadCount: 4,
              status: "active",
            },
            {
              id: "b",
              name: "Source B",
              count: 3,
              unreadCount: 3,
              status: "active",
            },
          ].reverse(),
          articlePage: { articles, total: 7, unreadTotal: 7 },
        }),
      });
    if (path === "/api/articles")
      return route.fulfill({
        json: wireFixture({
          articles: url.searchParams.has("cursor")
            ? [articles[6]]
            : articles.filter(
                (a) =>
                  !states.get(a.id)!.read &&
                  (!url.searchParams.has("subscription_id") ||
                    a.subscriptionIds.includes(
                      url.searchParams.get("subscription_id")!,
                    )),
              ),
          total: 7,
          unreadTotal: 7,
          olderCursor: url.searchParams.has("cursor") ? null : "next-page",
          newerCursor: url.searchParams.has("cursor") ? "first-page" : null,
        }),
      });
    const id = path.split("/")[3],
      article = articles.find((a) => a.id === id);
    if (article && path.endsWith("/reading") && req.method() === "GET")
      return route.fulfill({ json: states.get(id) });
    if (article && path.endsWith("/reading") && req.method() === "POST") {
      const command = req.postDataJSON();
      commands.push(command);
      const state = states.get(id)!;
      state.read = true;
      state.revision = "1";
      state.rating = command.rating;
      state.reason = command.reason;
      state.ratedAt = "2026-09-30T08:00:00Z";
      if (hold) await new Promise<void>((r) => (release = r));
      if (lose) {
        lose = false;
        return route.abort("connectionreset");
      }
      return route.fulfill({
        json: {
          operationId: command.operationId,
          articleId: id,
          state,
          undone: false,
        },
      });
    }
    if (article && path.endsWith("/chats")) {
      reads.push(id);
      return route.fulfill({
        json: [
          {
            id: `chat-${id}`,
            articleId: id,
            workspaceId: "ws",
            title: article.title,
            sourceUrl: article.url,
            createdAt: "2026-09-30T08:00:00Z",
            model: "fixture",
            promptVersion: "fixture",
            status: "completed",
            providerCalls: [],
            messages: [
              {
                id: `summary-${id}`,
                role: "assistant",
                purpose: "summary",
                phase: "generating",
                status: "complete",
                content: `# Summary ${id}\n\n**Useful engineering detail.**\n\n${"Exact explanation without truncating stored text. ".repeat(70)}\n\nEnd of the complete summary.`,
                createdAt: "2026-09-30T08:00:00Z",
              },
            ],
          },
        ],
      });
    }
    if (article && path.endsWith("/chat")) {
      paid++;
      return route.abort();
    }
    if (article && path === `/api/articles/${id}`)
      return route.fulfill({
        json: wireFixture({ ...article, read: states.get(id)!.read }),
      });
    if (path === "/api/ai/profile")
      return route.fulfill({
        json: {
          models: { summary: "deepseek-flash", verification: null },
          configured: true,
          enabled: true,
        },
      });
    return route.continue();
  });
  return {
    articles,
    commands,
    reads,
    states,
    paid: () => paid,
    hold: (value: boolean) => (hold = value),
    lose: () => (lose = true),
    release: () => release(),
  };
}

test("digest shows full saved summaries and keeps rating targets fixed during save", async ({
  page,
}) => {
  const f = await fixture(page);
  await page.goto("/reader");
  await page.getByRole("button", { name: "Reading mode", exact: true }).click();
  await expect(page).toHaveURL(/\/digest/);
  await expect(page.locator(".digest-source")).toHaveCount(1);
  const sources = page.locator(".reading-digest__source");
  await expect(sources).toHaveText(["Source A4", "Source B3"]);
  await expect(page.getByRole("button", { name: /Empty source/ })).toHaveCount(
    0,
  );
  await expect(page.locator(".digest-article")).toHaveCount(4);
  const card = page.getByRole("article", {
    name: "Digest article 1",
    exact: true,
  });
  const next = page.getByRole("article", {
    name: "Digest article 3",
    exact: true,
  });
  await expect(card.getByText("Summary 1", { exact: true })).toBeVisible();
  expect(f.reads).toContain("7");
  expect(f.paid()).toBe(0);
  const summary = card.locator(".digest-article__summary");
  await expect(summary).toContainText("End of the complete summary.");
  expect(
    await summary.evaluate((el) => el.scrollHeight <= el.clientHeight + 1),
  ).toBe(true);
  let before = await next.boundingBox();
  await card
    .getByRole("button", { name: "Rate 9 out of 10", exact: true })
    .click();
  await card.getByRole("textbox").fill("  Полезный механизм.\nНужны замеры.  ");
  const save = card.locator(".reading-reason__save");
  await save.scrollIntoViewIfNeeded();
  before = await next.boundingBox();
  const bounds = await save.boundingBox();
  f.hold(true);
  await save.click();
  await expect(save).toBeDisabled();
  await expect(save).toHaveAttribute("aria-busy", "true");
  await save.dispatchEvent("click");
  await expect.poll(() => f.commands.length).toBe(1);
  await expect(
    page.getByRole("button", { name: "Обновить подборку", exact: true }),
  ).toBeDisabled();
  expect(await save.boundingBox()).toEqual(bounds);
  expect(await next.boundingBox()).toEqual(before);
  f.release();
  await expect(page.locator('.digest-article[data-read="true"]')).toHaveCSS(
    "visibility",
    "hidden",
  );
  expect(await next.boundingBox()).toEqual(before);
  expect(f.commands[0].reason).toBe("  Полезный механизм.\nНужны замеры.  ");
  await page
    .getByRole("button", { name: "Обновить подборку", exact: true })
    .click();
  await expect(card).toHaveCount(0);
  expect(f.paid()).toBe(0);
});

test("digest preserves drafts and pagination, supports a full-text-free card and the existing discussion", async ({
  page,
}) => {
  const f = await fixture(page);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/digest?workspace=ws");
  const card = page.getByRole("article", {
    name: "Digest article 1",
    exact: true,
  });
  const reason = card.getByRole("textbox");
  await expect(reason).toBeHidden();
  await card
    .getByRole("button", { name: "Rate 7 out of 10", exact: true })
    .click();
  await reason.scrollIntoViewIfNeeded();
  await reason.fill("Черновик для статьи");
  await page.reload();
  await reason.scrollIntoViewIfNeeded();
  await expect(reason).toHaveValue("Черновик для статьи");
  await expect(
    card.getByRole("button", { name: "Rate 7 out of 10", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  const pending = page.getByRole("article", {
    name: "Digest article 3",
    exact: true,
  });
  await pending.scrollIntoViewIfNeeded();
  await expect(pending).toContainText("Полный текст ещё не получен");
  await expect(
    pending.getByRole("button", { name: "Весь пересказ", exact: true }),
  ).toHaveCount(0);
  expect(f.reads).not.toContain("3");
  await page.getByRole("button", { name: "Старее →", exact: true }).click();
  await expect(page).toHaveURL(/cursor=next-page/);
  await expect(page.locator(".digest-article")).toHaveCount(1);
  await page.goBack();
  await expect(page.locator(".digest-article")).toHaveCount(4);
  await card
    .getByRole("button", { name: "Читать и обсудить →", exact: true })
    .click();
  await expect(page).toHaveURL(/\/reading\?.*from=digest/);
  expect(f.paid()).toBe(0);
});

test("subscription selection scopes the request and restores drafts on Back", async ({
  page,
}) => {
  await fixture(page);
  await page.goto("/digest?workspace=ws&subscription=a");
  const card = page.getByRole("article", {
    name: "Digest article 1",
    exact: true,
  });
  await card
    .getByRole("button", { name: "Rate 7 out of 10", exact: true })
    .click();
  await card.getByRole("textbox").fill("Не потерять");
  await page
    .getByRole("navigation", { name: "Подписки для чтения" })
    .getByRole("button", { name: "Source B 3" })
    .click();
  await expect(page).toHaveURL(/subscription=b/);
  await expect(page.locator(".digest-article")).toHaveCount(3);
  await expect(card).toHaveCount(0);
  await page.goBack();
  await expect(card.getByRole("textbox")).toHaveValue("Не потерять");
});

test("switching sources shows pending feedback without moving navigation or issuing duplicate requests", async ({
  page,
}) => {
  await fixture(page);
  await page.goto("/digest?workspace=ws&subscription=a");
  await expect(page.locator(".digest-article")).toHaveCount(4);
  let requests = 0;
  let release!: () => void;
  await page.route("**/api/articles?**", async (route) => {
    if (
      new URL(route.request().url()).searchParams.get("subscription_id") === "b"
    ) {
      requests++;
      await new Promise<void>((resolve) => {
        release = resolve;
      });
    }
    await route.fallback();
  });
  const button = page
    .getByRole("navigation", { name: "Подписки для чтения" })
    .getByRole("button", { name: "Source B 3" });
  const bounds = await button.boundingBox();
  await button.click();
  await expect(page.locator(".reading-digest")).toHaveAttribute(
    "aria-busy",
    "true",
  );
  await expect(button).toBeDisabled();
  await button.dispatchEvent("click");
  expect(await button.boundingBox()).toEqual(bounds);
  await expect.poll(() => requests).toBe(1);
  release();
  await expect(page.locator(".digest-article")).toHaveCount(3);
  expect(await button.boundingBox()).toEqual(bounds);
});

test("complete summaries settle before any rating targets appear", async ({
  page,
}) => {
  await fixture(page);
  let release!: () => void;
  await page.route("**/api/articles/7/chats?**", async (route) => {
    await new Promise<void>((resolve) => {
      release = resolve;
    });
    await route.fallback();
  });
  await page.goto("/digest?workspace=ws&subscription=a");
  await expect.poll(() => !!release).toBe(true);
  await expect(page.locator(".reading-digest")).toHaveAttribute(
    "aria-busy",
    "true",
  );
  await expect(page.locator(".digest-article")).toHaveCount(0);
  const navigation = page.locator(".reading-digest__sources");
  const bounds = await navigation.boundingBox();
  release();
  await expect(page.locator(".digest-article")).toHaveCount(4);
  expect(await navigation.boundingBox()).toEqual(bounds);
  for (const width of [1280, 390]) {
    await page.setViewportSize({ width, height: 844 });
    const summaries = page.locator(".digest-article__summary");
    for (const summary of await summaries.all()) {
      expect(
        await summary.evaluate((el) => el.scrollHeight <= el.clientHeight + 1),
      ).toBe(true);
    }
  }
});

test("each digest card opens its own original in a new tab without changing feedback", async ({
  page,
}) => {
  const f = await fixture(page);
  await page.goto("/digest?workspace=ws&subscription=a");
  const card = page.getByRole("article", {
    name: "Digest article 1",
    exact: true,
  });
  const link = card.getByRole("link", { name: "Open original" });
  await expect(link).toHaveAttribute("href", "https://example.test/1");
  await expect(link).toHaveAttribute("target", "_blank");
  await expect(link).toHaveAttribute("rel", "noopener noreferrer");
  const title = card.locator("h3").first();
  const bounds = await title.boundingBox();
  await link.hover();
  expect(await title.boundingBox()).toEqual(bounds);
  await page
    .context()
    .route("https://example.test/**", (route) =>
      route.fulfill({ body: "Original article" }),
    );
  const popupPromise = page.waitForEvent("popup");
  await link.click();
  const popup = await popupPromise;
  await popup.waitForLoadState();
  expect(popup.url()).toBe("https://example.test/1");
  await popup.close();
  await expect(page).toHaveURL(/\/digest/);
  expect(f.commands).toHaveLength(0);
  expect(f.paid()).toBe(0);
  await page.setViewportSize({ width: 390, height: 844 });
  await link.scrollIntoViewIfNeeded();
  const box = (await link.boundingBox())!;
  expect(box.x + box.width).toBeLessThanOrEqual(390);
});

test("both reading modes keep the same back button coordinates and return destination", async ({
  page,
}) => {
  await fixture(page);
  for (const width of [1440, 390]) {
    await page.setViewportSize({ width, height: 900 });
    await page.goto("/digest?workspace=ws&subscription=a");
    const back = page.getByRole("button", { name: "← Back", exact: true });
    await expect(back).toBeVisible();
    const bounds = await back.boundingBox();
    await page
      .getByRole("article", { name: "Digest article 1", exact: true })
      .getByRole("button", { name: "Читать и обсудить →", exact: true })
      .click();
    await expect(page).toHaveURL(/\/reading/);
    expect(await back.boundingBox()).toEqual(bounds);
    await back.click();
    await expect(page).toHaveURL(/\/digest\?.*subscription=a/);
    expect(await back.boundingBox()).toEqual(bounds);
    await back.click();
    await expect(page).toHaveURL(/\/reader\?/);
  }
});

test("commit cards do not require scoring and keep hidden explanation and stable adjacent cards", async ({
  page,
}) => {
  const f = await fixture(page);
  f.articles[0].url = "https://github.com/chrisant996/clink/commit/123abc";
  await page.goto("/digest?workspace=ws");
  const card = page.getByRole("article", {
    name: "Digest article 1",
    exact: true,
  });
  await expect(card.getByRole("textbox")).toBeHidden();
  const next = page.getByRole("article", {
    name: "Digest article 3",
    exact: true,
  });
  const save = card.locator(".reading-reason__save");
  await expect(save).toBeEnabled();
  await save.scrollIntoViewIfNeeded();
  const before = await next.boundingBox();
  await save.click();
  await expect(page.locator('.digest-article[data-read="true"]')).toHaveCSS(
    "visibility",
    "hidden",
  );
  expect(await next.boundingBox()).toEqual(before);
  expect(f.commands[0].rating).toBeNull();
});

test("late extracted terms scroll in their reserved slot without moving rating targets", async ({
  page,
}) => {
  await fixture(page);
  let release!: () => void;
  const pending = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/api/articles/1/definitions*", async (route) => {
    await pending;
    await route.fulfill({
      json: {
        channel: {
          channel: "reading_data_news",
          configured: false,
          conflicts: 0,
          coverageNote: "",
          definitions: 0,
          generationAllowed: true,
          historyIncomplete: false,
          indexReady: true,
          pending: 0,
          posts: 0,
          revision: 0,
          syncPending: false,
          unindexed: 0,
        },
        known: [],
        job: {
          id: "terms-1",
          articleId: "1",
          workspaceId: "ws",
          model: "flash",
          promptVersion: "fixture",
          status: "completed",
          result: {
            entities: [
              {
                name: "Kafka",
                kind: "product",
                explanation: "Платформа потоковых событий. ".repeat(300),
                insufficientContext: false,
              },
            ],
          },
        },
      },
    });
  });
  await page.goto("/digest?workspace=ws");
  const card = page.getByRole("article", {
    name: "Digest article 1",
    exact: true,
  });
  const score = card.getByRole("button", {
    name: "Rate 7 out of 10",
    exact: true,
  });
  const save = card.locator(".reading-reason__save");
  await expect(score).toBeEnabled();
  const scoreBefore = await score.boundingBox(),
    saveBefore = await save.boundingBox();
  release();
  await expect(
    card.getByRole("region", { name: "Извлечённые термины" }),
  ).toContainText("Платформа потоковых событий.");
  expect(await score.boundingBox()).toEqual(scoreBefore);
  expect(await save.boundingBox()).toEqual(saveBefore);
  const content = card.locator(".digest-article__terms > div");
  expect(
    await content.evaluate((el) => el.scrollHeight > el.clientHeight),
  ).toBe(true);
});

test("unknown rating reveals its explanation without moving controls and stores no numeric label", async ({
  page,
}) => {
  const f = await fixture(page);
  await page.goto("/digest?workspace=ws");
  const card = page.getByRole("article", {
    name: "Digest article 1",
    exact: true,
  });
  await card.scrollIntoViewIfNeeded();
  const save = card.locator(".reading-reason__save");
  await save.scrollIntoViewIfNeeded();
  const unknown = card.getByRole("button", { name: "Не знаю", exact: true });
  // Put both targets in the scroll viewport before comparing coordinates;
  // Playwright otherwise scrolls the partially clipped unknown button on click.
  await unknown.scrollIntoViewIfNeeded();
  const bounds = await save.boundingBox();
  await expect(card.getByRole("textbox")).toBeHidden();
  await unknown.click();
  await expect(card.getByRole("textbox")).toBeVisible();
  expect(await save.boundingBox()).toEqual(bounds);
  await card
    .getByRole("textbox")
    .fill("Не могу оценить без дополнительных данных");
  await save.click();
  await expect.poll(() => f.commands.length).toBe(1);
  expect(f.commands[0].rating).toBeNull();
  expect(f.commands[0].reason).toBe(
    "Не могу оценить без дополнительных данных",
  );
});
