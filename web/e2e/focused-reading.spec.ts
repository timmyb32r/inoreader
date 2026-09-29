import { expect, test, type Page } from "@playwright/test";
import { wireFixture } from "./wire-fixtures.mjs";

async function fixture(page: Page) {
  const articles = [1, 2, 3].map((id) => ({
    id: String(id),
    title: `Article ${id}`,
    url: `https://example.test/${id}`,
    source: "Example",
    excerpt: "Article introduction",
    body: [
      "The original article describes a concrete engineering trade-off. ".repeat(
        15,
      ),
    ],
    fullText: "ready",
    savedAt: `2026-09-29T10:00:0${4 - id}Z`,
    read: false,
    later: false,
  }));
  const states = articles.map(() => ({
    revision: "0",
    read: false,
    rating: null as number | null,
    ratedAt: null as string | null,
  }));
  const chat = (id: string) => ({
    id: `chat-${id}`,
    articleId: id,
    workspaceId: "ws",
    title: `Article ${id}`,
    sourceUrl: "https://example.test",
    createdAt: "2026-09-29T12:00:00Z",
    model: "fixture",
    promptVersion: "fixture",
    status: "completed",
    providerCalls: [],
    messages: [
      {
        id: `summary-${id}`,
        role: "assistant",
        purpose: "summary",
        phase: "verifying",
        status: "complete",
        content: `Prepared summary for ${id}`,
        createdAt: "2026-09-29T12:00:00Z",
      },
    ],
  });
  const calls = { complete: 0, ordinary: 0, generation: 0, next: 0 };
  const receipts = new Map<string, any>();
  let delay: Promise<void> | undefined;
  let lost = false;
  let nextFails = false;
  let undoFails = false;
  const batch = (items = articles.filter((a) => !a.read)) => ({
    articles: items,
    total: items.length,
    unreadTotal: articles.filter((a) => !a.read).length,
  });
  await page.route("**/api/**", async (route) => {
    const url = new URL(route.request().url()),
      path = url.pathname;
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
    if (path === "/api/ai/profile")
      return route.fulfill({
        json: {
          models: { summary: "deepseek-flash", verification: "deepseek-flash" },
          configured: true,
          enabled: true,
        },
      });
    if (path === "/api/articles")
      return route.fulfill({ json: wireFixture(batch()) });
    const match = /^\/api\/articles\/(1|2|3)(.*)$/.exec(path);
    if (match) {
      const i = Number(match[1]) - 1,
        suffix = match[2];
      if (suffix === "")
        return route.fulfill({ json: wireFixture(articles[i]) });
      if (suffix === "/chats") return route.fulfill({ json: [chat(match[1])] });
      if (suffix === "/chat") {
        calls.generation++;
        return route.fulfill({ json: chat(match[1]) });
      }
      if (suffix === "/state") {
        calls.ordinary++;
        const patch = route.request().postDataJSON();
        Object.assign(articles[i], patch);
        states[i].read = articles[i].read;
        states[i].revision = String(Number(states[i].revision) + 1);
        return route.fulfill({ json: wireFixture(articles[i]) });
      }
      if (suffix === "/reading/next") {
        calls.next++;
        if (nextFails)
          return route.fulfill({
            status: 503,
            json: { message: "Next article unavailable" },
          });
        return route.fulfill({
          json: wireFixture(
            batch(articles.slice(i + 1).filter((a) => !a.read)),
          ),
        });
      }
      if (suffix === "/reading" && route.request().method() === "GET")
        return route.fulfill({ json: states[i] });
      if (suffix === "/reading") {
        calls.complete++;
        await delay;
        const command = route.request().postDataJSON();
        if (!receipts.has(command.operationId)) {
          if (command.expectedRevision !== states[i].revision || states[i].read)
            return route.fulfill({
              status: 409,
              json: { message: "Article changed in another tab" },
            });
          if (
            !Number.isInteger(command.rating) ||
            command.rating < 1 ||
            command.rating > 10
          )
            return route.fulfill({
              status: 422,
              json: { message: "Rating required" },
            });
          const previous = { ...states[i] };
          states[i] = {
            revision: String(Number(states[i].revision) + 1),
            read: true,
            rating: command.rating,
            ratedAt: "2026-09-29T12:00:00Z",
          };
          articles[i].read = true;
          receipts.set(command.operationId, {
            previous,
            result: {
              operationId: command.operationId,
              articleId: match[1],
              state: { ...states[i] },
              undone: false,
            },
          });
        }
        if (lost) {
          lost = false;
          return route.abort();
        }
        return route.fulfill({
          json: receipts.get(command.operationId).result,
        });
      }
      if (suffix.endsWith("/undo")) {
        if (undoFails)
          return route.fulfill({
            status: 409,
            json: { message: "Later change prevents undo" },
          });
        const op = suffix.split("/")[2],
          receipt = receipts.get(op);
        if (!receipt)
          return route.fulfill({ status: 404, json: { message: "Not found" } });
        states[i] = {
          ...receipt.previous,
          revision: String(Number(states[i].revision) + 1),
        };
        articles[i].read = states[i].read;
        receipt.result = { ...receipt.result, state: states[i], undone: true };
        return route.fulfill({ json: receipt.result });
      }
    }
    const polling = /^\/api\/ai\/chats\/chat-(\d)(\/changes)?$/.exec(path);
    if (polling)
      return route.fulfill({
        json: polling[2]
          ? { revision: "1", chat: chat(polling[1]) }
          : chat(polling[1]),
      });
    return route.continue();
  });
  return {
    chat,
    articles,
    states,
    calls,
    receipts,
    hold: () => {
      let release!: () => void;
      delay = new Promise<void>((r) => (release = r));
      return () => {
        release();
        delay = undefined;
      };
    },
    loseNext: () => {
      lost = true;
    },
    failNext: (v: boolean) => {
      nextFails = v;
    },
    failUndo: () => {
      undoFails = true;
    },
  };
}
const heading = (page: Page) =>
  page.locator(".focused-reading .reader-body h1");
const next = (page: Page) =>
  page.getByRole("button", { name: "Read & next →", exact: true });
const rate = (page: Page, value: number) =>
  page.getByRole("button", { name: `Rate ${value} out of 10`, exact: true });

test("dedicated mode requires rating, keeps geometry, completes once, resets score, and undoes without paid generation", async ({
  page,
}) => {
  const f = await fixture(page);
  await page.goto("/reader");
  await page.getByRole("button", { name: "Reading mode", exact: true }).click();
  await expect(heading(page)).toHaveText("Article 1");
  await expect(
    page.getByText("Prepared summary for 1", { exact: true }),
  ).toBeVisible();
  await expect(page.locator(".article-list")).toHaveCount(0);
  await expect(page.getByRole("dialog", { name: /Article chat/ })).toHaveCount(
    0,
  );
  await expect(next(page)).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Mark read", exact: true }),
  ).toHaveCount(0);
  const before = await next(page).boundingBox(),
    composer = await page
      .getByRole("textbox", { name: "Message DeepSeek" })
      .boundingBox();
  await rate(page, 8).click();
  expect(await next(page).boundingBox()).toEqual(before);
  expect(
    await page.getByRole("textbox", { name: "Message DeepSeek" }).boundingBox(),
  ).toEqual(composer);
  const release = f.hold();
  await next(page).click();
  await expect(next(page)).toBeDisabled();
  await expect(next(page)).toHaveAttribute("aria-busy", "true");
  expect(await next(page).boundingBox()).toEqual(before);
  await page.keyboard.press("j");
  await expect(heading(page)).toHaveText("Article 1");
  expect(f.calls.complete).toBe(1);
  release();
  await expect(heading(page)).toHaveText("Article 2");
  await expect(next(page)).toBeDisabled();
  expect(f.states[0].rating).toBe(8);
  expect(f.calls.ordinary).toBe(0);
  expect(f.calls.generation).toBe(0);
  await page.getByRole("button", { name: "Undo last read" }).click();
  await expect(heading(page)).toHaveText("Article 1");
  expect(f.states[0].rating).toBe(null);
  expect(f.articles[0].read).toBe(false);
  await expect(next(page)).toBeDisabled();
});

test("lost response survives reload and replays the identical completion instead of scoring twice", async ({
  page,
}) => {
  const f = await fixture(page);
  await page.goto("/reading?workspace=ws&article=1");
  await expect(heading(page)).toHaveText("Article 1");
  await rate(page, 7).click();
  f.loseNext();
  await next(page).click();
  await expect(
    page.getByRole("button", { name: "Retry saving", exact: true }),
  ).toBeEnabled();
  expect(f.receipts.size).toBe(1);
  await page.reload();
  await expect(
    page.getByRole("button", { name: "Retry saving", exact: true }),
  ).toBeEnabled();
  await page.getByRole("button", { name: "Retry saving", exact: true }).click();
  await expect(heading(page)).toHaveText("Article 2");
  expect(f.calls.complete).toBe(2);
  expect(f.receipts.size).toBe(1);
  expect(f.states[0].revision).toBe("1");
});

test("drafts follow their article, browser history is read-only, and no-fulltext article hides the chat", async ({
  page,
}) => {
  const f = await fixture(page);
  f.articles[1].fullText = "failed";
  await page.goto("/reading?workspace=ws&article=1");
  await expect(heading(page)).toHaveText("Article 1");
  await page
    .getByRole("textbox", { name: "Message DeepSeek" })
    .fill("My unsent question");
  await rate(page, 6).click();
  await next(page).click();
  await expect(heading(page)).toHaveText("Article 2");
  await expect(
    page.getByRole("textbox", { name: "Message DeepSeek" }),
  ).toHaveCount(0);
  await expect(
    page.getByText("Prepared summary for 1", { exact: true }),
  ).toHaveCount(0);
  await page.goBack();
  await expect(heading(page)).toHaveText("Article 1");
  await expect(
    page.getByRole("textbox", { name: "Message DeepSeek" }),
  ).toHaveValue("My unsent question");
  expect(f.calls.complete).toBe(1);
  expect(f.calls.generation).toBe(0);
  await page.goForward();
  await expect(heading(page)).toHaveText("Article 2");
  await page.reload();
  await expect(heading(page)).toHaveText("Article 2");
  expect(f.calls.ordinary).toBe(0);
});

test("next-load failure retains committed rating; undo conflicts are explicit; narrow controls stay in place", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const f = await fixture(page);
  await page.goto("/reading?workspace=ws&article=1");
  await expect(heading(page)).toHaveText("Article 1");
  const before = await next(page).boundingBox();
  await rate(page, 9).click();
  expect(await next(page).boundingBox()).toEqual(before);
  f.failNext(true);
  await next(page).click();
  await expect(
    page.getByText("Next article unavailable", { exact: true }),
  ).toBeVisible();
  expect(f.states[0].rating).toBe(9);
  const continued = page.getByRole("button", {
    name: "Continue →",
    exact: true,
  });
  expect(await continued.boundingBox()).toEqual(before);
  f.failNext(false);
  await continued.click();
  await expect(heading(page)).toHaveText("Article 2");
  f.failUndo();
  await page.getByRole("button", { name: "Undo last read" }).click();
  await expect(
    page.getByText("Later change prevents undo", { exact: true }),
  ).toBeVisible();
  expect(f.articles[0].read).toBe(true);
  expect(await next(page).boundingBox()).toEqual(before);
});

test("keyboard rating does not commit on exit; queue end keeps undo and desktop/narrow panels fit", async ({
  page,
}) => {
  const f = await fixture(page);
  await page.goto("/reading?workspace=ws&article=3");
  await expect(heading(page)).toHaveText("Article 3");
  await expect(
    page.getByText("Prepared summary for 3", { exact: true }),
  ).toBeVisible();
  await page.screenshot({ path: "/tmp/focused-reading-desktop.png" });
  await rate(page, 6).focus();
  await page.keyboard.press("Enter");
  await expect(rate(page, 6)).toHaveAttribute("aria-pressed", "true");
  expect(f.calls.complete).toBe(0);
  await page
    .getByRole("button", { name: "← Back to Feed", exact: true })
    .click();
  await expect(page).toHaveURL(/\/reader\?/);
  expect(f.articles[2].read).toBe(false);
  expect(f.states[2].rating).toBe(null);
  await page.goBack();
  await expect(heading(page)).toHaveText("Article 3");
  await expect(next(page)).toBeDisabled();
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({ path: "/tmp/focused-reading-mobile.png" });
  await expect(
    page.getByRole("button", { name: "Open navigation", exact: true }),
  ).toHaveCount(0);
  const bounds = await next(page).boundingBox();
  expect(bounds!.y + bounds!.height).toBeLessThan(844);
  await page.getByRole("button", { name: "Assistant", exact: true }).click();
  await expect(
    page.getByText("Prepared summary for 3", { exact: true }),
  ).toBeVisible();
  expect(await next(page).boundingBox()).toEqual(bounds);
  await rate(page, 4).click();
  await next(page).click();
  await expect(
    page.getByRole("heading", { name: "All caught up" }),
  ).toBeVisible();
  await expect(next(page)).toBeDisabled();
  await page.getByRole("button", { name: "Undo last read" }).click();
  await expect(heading(page)).toHaveText("Article 3");
  expect(f.articles[2].read).toBe(false);
  expect(f.states[2].rating).toBe(null);
});

test("a previous rating is visible but cannot enable a second completion under a repeated click", async ({
  page,
}) => {
  const f = await fixture(page);
  f.states[1].rating = 8;
  f.states[1].ratedAt = "2026-09-28T12:00:00Z";
  await page.goto("/reading?workspace=ws&article=1");
  await rate(page, 7).click();
  await next(page).click();
  await expect(heading(page)).toHaveText("Article 2");
  await expect(
    page.getByText("Previous rating: 8/10 · choose a rating to finish.", {
      exact: true,
    }),
  ).toBeVisible();
  await expect(next(page)).toBeDisabled();
  expect(f.calls.complete).toBe(1);
  expect(f.states[1].rating).toBe(8);
});

test("Terms tab opens directly, deduplicates loading and keeps drafts and controls stable", async ({
  page,
}) => {
  await fixture(page);
  let release!: () => void;
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  let reads = 0;
  await page.route("**/api/articles/1/definitions*", async (route) => {
    expect(route.request().method()).toBe("GET");
    reads++;
    await held;
    await route.fulfill({
      json: {
        channel: {
          channel: "reading_data_news",
          configured: false,
          botUsername: null,
          indexReady: true,
          revision: 1,
          posts: 1529,
          definitions: 836,
          unindexed: 0,
          pending: 0,
          conflicts: 0,
          lastPoll: null,
          lastHistory: null,
          pollError: null,
          historyError: null,
          historyIncomplete: false,
          coverageNote: "",
          syncPending: false,
          generationAllowed: true,
        },
        known: [],
        job: {
          id: "terms-1",
          workspaceId: "ws",
          articleId: "1",
          model: "flash",
          promptVersion: "fixture",
          status: "completed",
          result: {
            entities: [
              {
                name: "Kafka",
                kind: "product",
                explanation: "A platform for event streams. ".repeat(80),
                insufficientContext: false,
              },
            ],
          },
        },
      },
    });
  });
  await page.goto("/reading?workspace=ws&article=1");
  const composer = page.getByRole("textbox", { name: "Message DeepSeek" });
  await composer.fill("My question stays here");
  const footer = await next(page).boundingBox(),
    input = await composer.boundingBox();
  await expect(
    page.getByRole("tab", { name: "Discussion", exact: true }),
  ).toHaveCount(0);
  const terms = page.getByRole("tab", { name: "Terms", exact: true });
  await expect(terms).toBeEnabled();
  await terms.click();
  await expect(terms).toHaveAttribute("aria-selected", "true");
  await expect(terms).toHaveAttribute("aria-busy", "true");
  await expect(page.locator(".glossary-panel--embedded")).toBeVisible();
  await terms.click();
  expect(reads).toBe(1);
  expect(await next(page).boundingBox()).toEqual(footer);
  release();
  await expect(
    page
      .locator(".glossary-paragraph")
      .filter({ hasText: "A platform for event streams." }),
  ).toBeVisible();
  expect(await next(page).boundingBox()).toEqual(footer);
  const panel = page.locator(".glossary-panel--embedded");
  const content = panel.locator(".glossary-content");
  for (const viewport of [
    { width: 1280, height: 900 },
    { width: 390, height: 844 },
  ]) {
    await page.setViewportSize(viewport);
    const box = await panel.boundingBox(),
      text = await content.boundingBox();
    const bottom = await panel.locator("footer").boundingBox();
    expect(text!.height).toBeGreaterThan(box!.height * 0.5);
    expect(bottom!.height).toBe(68);
    expect(text!.y + text!.height).toBeLessThanOrEqual(bottom!.y + 1);
    await content.evaluate((element) => {
      element.scrollTop = element.scrollHeight;
    });
    expect(
      await content.evaluate((element) => element.scrollTop),
    ).toBeGreaterThan(0);
    expect(await panel.locator("footer").boundingBox()).toEqual(bottom);
  }
  await page.setViewportSize({ width: 1280, height: 720 });
  await page.getByRole("tab", { name: "Summary", exact: true }).click();
  await expect(composer).toHaveValue("My question stays here");
  expect(await composer.boundingBox()).toEqual(input);
  await terms.click();
  await expect(
    page
      .locator(".glossary-paragraph")
      .filter({ hasText: "A platform for event streams." }),
  ).toBeVisible();
  expect(reads).toBe(1);
});

test("Summary is the complete chat: sending a question keeps summary, question and answer together", async ({
  page,
}) => {
  const f = await fixture(page);
  await page.route("**/api/ai/chats/chat-1/messages", async (route) => {
    const content = route.request().postDataJSON().content;
    const c = f.chat("1");
    await route.fulfill({
      json: {
        ...c,
        messages: [
          ...c.messages,
          {
            id: "q",
            role: "user",
            purpose: "chat",
            status: "complete",
            content,
            createdAt: c.createdAt,
          },
          {
            id: "a",
            role: "assistant",
            purpose: "chat",
            status: "complete",
            content: "Here is the explanation.",
            createdAt: c.createdAt,
          },
        ],
      },
    });
  });
  await page.goto("/reading?workspace=ws&article=1");
  await page
    .getByRole("textbox", { name: "Message DeepSeek" })
    .fill("Explain this trade-off");
  await page.getByRole("button", { name: "Send", exact: true }).click();
  await expect(
    page.getByRole("tab", { name: "Summary", exact: true }),
  ).toHaveAttribute("aria-selected", "true");
  await expect(
    page.getByText("Prepared summary for 1", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("Explain this trade-off", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("Here is the explanation.", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("tab", { name: "Discussion", exact: true }),
  ).toHaveCount(0);
});
