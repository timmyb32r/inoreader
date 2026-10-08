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
    reason: null as string | null,
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
        await delay;
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
            !("rating" in command) ||
            (command.rating !== null &&
              (!Number.isInteger(command.rating) ||
                command.rating < 1 ||
                command.rating > 10))
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
            reason: command.reason,
            ratedAt: "2026-09-29T12:00:00Z",
          };
          articles[i].read = true;
          receipts.set(command.operationId, {
            previous,
            command,
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
const reasonDialog = (page: Page) =>
  page.getByRole("dialog", { name: /^Почему \d+ из 10\?$/ });
const save = (page: Page) =>
  reasonDialog(page).getByRole("button", {
    name: "Save & next →",
    exact: true,
  });
const rate = (page: Page, value: number) =>
  page.getByRole("button", { name: `Rate ${value} out of 10`, exact: true });

test("dedicated mode requires rating, keeps geometry, completes once, resets score, and undoes without paid generation", async ({
  page,
}) => {
  const f = await fixture(page);
  await page.goto("/reader");
  await page.getByRole("button", { name: "One by one", exact: true }).click();
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
  await save(page).click();
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
  const reason = "  Полезные примеры CDC\nНо без замеров.  ";
  await reasonDialog(page).getByRole("textbox").fill(reason);
  await save(page).click();
  await expect(page.locator(".focused-reading__next")).toBeEnabled();
  expect(f.receipts.size).toBe(1);
  await page.reload();
  await expect(page.locator(".focused-reading__next")).toBeEnabled();
  await page.locator(".focused-reading__next").click();
  await expect(reasonDialog(page).getByRole("textbox")).toHaveValue(reason);
  await expect(reasonDialog(page).getByRole("textbox")).toBeDisabled();
  await reasonDialog(page)
    .getByRole("button", { name: "Retry saving", exact: true })
    .click();
  await expect(heading(page)).toHaveText("Article 2");
  expect(f.calls.complete).toBe(2);
  expect(f.receipts.size).toBe(1);
  expect(f.states[0].revision).toBe("1");
  expect(f.states[0].reason).toBe(reason);
  expect([...f.receipts.values()][0].command.reason).toBe(reason);
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
  await save(page).click();
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
  await save(page).click();
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
  await page.getByRole("button", { name: "← Back", exact: true }).click();
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
  await save(page).click();
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
  await save(page).click();
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
  await page.getByRole("tab", { name: "Summary / Chat", exact: true }).click();
  await expect(composer).toHaveValue("My question stays here");
  expect(await composer.boundingBox()).not.toBeNull();
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
  await page.getByRole("textbox", { name: "Message DeepSeek" }).press("Enter");
  await expect(
    page.getByRole("tab", { name: "Summary / Chat", exact: true }),
  ).toHaveAttribute("aria-selected", "false");
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

test("reason confirmation retains exact draft on cancel/reload, saves once with stable controls and clears for next article", async ({
  page,
}) => {
  const f = await fixture(page);
  await page.goto("/reading?workspace=ws&article=1");
  await rate(page, 8).click();
  const footer = await next(page).boundingBox();
  await next(page).click();
  const dialog = reasonDialog(page),
    field = dialog.getByRole("textbox");
  await expect(dialog).toBeVisible();
  await expect(field).toBeFocused();
  await expect(field).toHaveAttribute("autocomplete", "none");
  await expect(field).toHaveCSS("resize", "none");
  await expect(field).toHaveCSS("font-size", "16px");
  expect(f.calls.complete).toBe(0);
  const reason = "  Практичный CDC 中文 🦆\nНе хватает сравнения стоимости.  ";
  const submit = await save(page).boundingBox();
  await field.fill(reason);
  expect(await next(page).boundingBox()).toEqual(footer);
  expect(await save(page).boundingBox()).toEqual(submit);
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(next(page)).toBeFocused();
  expect(f.articles[0].read).toBe(false);
  await next(page).click();
  await expect(field).toHaveValue(reason);
  await dialog.getByRole("button", { name: "Назад к статье" }).click();
  await page.reload();
  await rate(page, 8).click();
  await next(page).click();
  await expect(field).toHaveValue(reason);
  const before = await save(page).boundingBox();
  const release = f.hold();
  await save(page).dblclick();
  await expect(save(page)).toHaveAttribute("aria-busy", "true");
  await expect(field).toBeDisabled();
  await expect(
    dialog.getByRole("button", { name: "Close dialog" }),
  ).toBeDisabled();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeVisible();
  expect(await save(page).boundingBox()).toEqual(before);
  expect(await next(page).boundingBox()).toEqual(footer);
  expect(f.calls.complete).toBe(1);
  release();
  await expect(heading(page)).toHaveText("Article 2");
  await expect(dialog).toHaveCount(0);
  expect(f.states[0].reason).toBe(reason);
  await rate(page, 5).click();
  await next(page).click();
  await expect(field).toHaveValue("");
  await save(page).click();
  await expect(heading(page)).toHaveText("Article 3");
  expect(f.states[1].reason).toBeNull();
  await page.getByRole("button", { name: "Undo last read" }).click();
  await expect(heading(page)).toHaveText("Article 2");
  expect(f.states[1].reason).toBeNull();
});

test("narrow reason dialog does not overlap the initiating button; failure keeps draft and fixed targets", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const f = await fixture(page);
  await page.goto("/reading?workspace=ws&article=1");
  await rate(page, 3).click();
  const footer = await next(page).boundingBox();
  await next(page).click();
  const dialog = reasonDialog(page),
    field = dialog.getByRole("textbox");
  const before = await save(page).boundingBox();
  expect(before!.y + before!.height).toBeLessThan(footer!.y);
  expect(await next(page).boundingBox()).toEqual(footer);
  await field.fill("Не хватает технических деталей.");
  f.states[0].revision = "9";
  await save(page).click();
  await expect(
    dialog.getByText("Article changed in another tab", { exact: true }),
  ).toBeVisible();
  expect(await save(page).boundingBox()).toEqual(before);
  expect(await next(page).boundingBox()).toEqual(footer);
  await expect(field).toHaveValue("Не хватает технических деталей.");
  await expect(field).toBeEnabled();
  expect(f.articles[0].read).toBe(false);
  await page.screenshot({ path: "/tmp/rating-reason-mobile.png" });
  await dialog.getByRole("button", { name: "Назад к статье" }).click();
  await expect(next(page)).toBeFocused();
});

test("explicit unknown completes without a numeric training label and preserves footer coordinates", async ({
  page,
}) => {
  const f = await fixture(page);
  await page.goto("/reading?workspace=ws&article=1");
  const before = await next(page).boundingBox();
  await expect(
    page.getByRole("link", { name: "Wiki", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Copy title", exact: true }),
  ).toBeVisible();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.getByRole("button", { name: "Не знаю", exact: true }).click();
  expect(await next(page).boundingBox()).toEqual(before);
  await next(page).click();
  const dialog = page.getByRole("dialog", { name: "Без числовой оценки" });
  await dialog.getByRole("textbox").fill("Пока недостаточно контекста");
  await dialog
    .getByRole("button", { name: "Save & next →", exact: true })
    .click();
  await expect(heading(page)).toHaveText("Article 2");
  expect(f.states[0].rating).toBeNull();
  expect(f.states[0].reason).toBe("Пока недостаточно контекста");
  expect(f.calls.complete).toBe(1);
  expect(await next(page).boundingBox()).toEqual(before);
  await page.getByRole("button", { name: "Undo last read" }).click();
  await expect(heading(page)).toHaveText("Article 1");
  expect(f.articles[0].read).toBe(false);
});

test("title copy retains the exact source title and stable control geometry", async ({
  page,
}) => {
  await fixture(page);
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "clipboard", {
      value: {
        writeText: async (text: string) => {
          (window as any).copiedTitle = text;
        },
      },
    });
  });
  await page.goto("/reading?workspace=ws&article=1");
  const button = page.getByRole("button", { name: "Copy title", exact: true });
  const before = await button.boundingBox();
  await button.click();
  await expect(button).toHaveAttribute("data-copy-state", "copied");
  expect(await page.evaluate(() => (window as any).copiedTitle)).toBe(
    "Article 1",
  );
  expect(await button.boundingBox()).toEqual(before);
});

for (const width of [1440, 390]) {
  test(`smart reading opens ranked original, chat and ratings directly at ${width}px`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 900 });
    const f = await fixture(page);
    let requests = 0;
    await page.route("**/api/ai/smart-feed?*", async (route) => {
      requests++;
      await route.fulfill({
        json: {
          profile: null,
          total: 3,
          scored: 3,
          failed: 0,
          nextCursor: null,
          articles: [...f.articles]
            .reverse()
            .filter((article) => !article.read)
            .map((article) => ({
              id: article.id,
              title: article.title,
              excerpt: article.excerpt,
              prediction: {
                score: 9,
                confidence: "high",
                reason: "Useful engineering",
              },
              error: null,
            })),
        },
      });
    });
    await page.goto("/reader");
    await page
      .getByRole("button", { name: "Умный режим чтения", exact: true })
      .click();
    await expect(heading(page)).toHaveText("Article 3");
    await expect(page).toHaveURL(/article=3/);
    await expect(page).toHaveURL(/from=smart/);
    if (width === 390)
      await page
        .getByRole("button", { name: "Assistant", exact: true })
        .click();
    await expect(
      page.getByText("Prepared summary for 3", { exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole("textbox", { name: "Message DeepSeek" }),
    ).toBeVisible();
    const before = await next(page).boundingBox();
    await rate(page, 8).click();
    expect(await next(page).boundingBox()).toEqual(before);
    await next(page).click();
    await save(page).click();
    await expect(heading(page)).toHaveText("Article 2");
    await expect(next(page)).toBeDisabled();
    expect(f.states[2].rating).toBe(8);
    expect(f.calls.generation).toBe(0);
    expect(f.calls.next).toBe(0);
    expect(requests).toBe(2);
  });
}

for (const width of [1440, 390]) {
  test(`all project commits span pages, retain paragraph controls, discuss and read only their snapshot at ${width}px`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 900 });
    const f = await fixture(page);
    f.articles[0].url = "https://github.com/acme/engine/commit/a";
    f.articles[1].url = "https://github.com/acme/engine/commit/b";
    let release!: () => void;
    const delayed = new Promise<void>((resolve) => {
      release = resolve;
    });
    let pages = 0;
    await page.route("**/api/articles/1/reading/commits?*", async (route) => {
      pages++;
      const older = new URL(route.request().url()).searchParams.has("cursor");
      await route.fulfill({
        json: wireFixture({
          articles: [f.articles[older ? 1 : 0]],
          total: 2,
          unreadTotal: 2,
          olderCursor: older ? null : "another-page",
          newerCursor: null,
        }),
      });
    });
    await page.route("**/api/articles/2/chats?*", async (route) => {
      await delayed;
      const chat = f.chat("2");
      chat.messages[0].content =
        "# Title\n\nIntro\n\n**TL;DR:** A complete second commit paragraph.";
      await route.fulfill({ json: [chat] });
    });
    await page.goto("/reading?workspace=ws&article=1");
    const group = page.getByRole("region", {
      name: "Непрочитанные коммиты проекта",
    });
    await expect(
      group.getByRole("heading", { name: "Article 2", exact: true }),
    ).toBeVisible();
    const discuss = group.getByRole("button", { name: "Обсудить с ИИ" }).nth(1);
    const before = await discuss.boundingBox();
    const finish = page.getByRole("button", {
      name: "Все прочитаны →",
      exact: true,
    });
    const footer = await finish.boundingBox();
    release();
    await expect(
      group.getByText("A complete second commit paragraph.", { exact: true }),
    ).toBeVisible();
    expect(await discuss.boundingBox()).toEqual(before);
    expect(await finish.boundingBox()).toEqual(footer);
    expect(pages).toBeGreaterThanOrEqual(2);
    await discuss.click();
    if (width === 390)
      await expect(
        page.getByRole("textbox", { name: "Message DeepSeek" }),
      ).toBeVisible();
    await expect(page.locator(".focused-reading__chat")).toContainText(
      "A complete second commit paragraph.",
    );
    const unblock = f.hold();
    await finish.click();
    await expect(finish).toBeDisabled();
    await expect(finish).toHaveAttribute("aria-busy", "true");
    expect(await finish.boundingBox()).toEqual(footer);
    await finish.evaluate((button: HTMLButtonElement) => button.click());
    expect(f.calls.ordinary).toBe(2);
    unblock();
    await expect(heading(page)).toHaveText("Article 3");
    expect(f.articles[0].read).toBe(true);
    expect(f.articles[1].read).toBe(true);
    expect(f.articles[2].read).toBe(false);
    expect(f.calls.complete).toBe(0);
    expect(f.states[0].rating).toBeNull();
    expect(f.states[1].rating).toBeNull();
  });
}

test("project commit partial failure keeps successful reads, identifies failed row and retries without ratings", async ({
  page,
}) => {
  const f = await fixture(page);
  f.articles[0].url = "https://github.com/acme/engine/commit/a";
  f.articles[1].url = "https://github.com/acme/engine/commit/b";
  await page.route("**/api/articles/1/reading/commits?*", (route) =>
    route.fulfill({
      json: wireFixture({
        articles: f.articles.slice(0, 2),
        total: 2,
        unreadTotal: 2,
      }),
    }),
  );
  let fail = true;
  await page.route("**/api/articles/2/state?*", async (route) => {
    if (fail)
      return route.fulfill({
        status: 503,
        json: { message: "Temporarily unavailable" },
      });
    await route.fallback();
  });
  await page.goto("/reading?workspace=ws&article=1");
  const finish = page.getByRole("button", {
    name: "Все прочитаны →",
    exact: true,
  });
  await expect(finish).toBeEnabled();
  await finish.click();
  await expect(
    page.getByText("Не удалось отметить прочитанным. Повторите действие.", {
      exact: true,
    }),
  ).toBeVisible();
  expect(f.articles[0].read).toBe(true);
  expect(f.articles[1].read).toBe(false);
  expect(f.calls.complete).toBe(0);
  fail = false;
  await finish.click();
  await expect(heading(page)).toHaveText("Article 3");
  expect(f.articles[1].read).toBe(true);
  expect(f.states[0].rating).toBeNull();
  expect(f.states[1].rating).toBeNull();
});

test("empty project snapshot can advance without inventing a rating or marking other articles", async ({
  page,
}) => {
  const f = await fixture(page);
  f.articles[0].url = "https://github.com/acme/engine/commit/a";
  await page.route("**/api/articles/1/reading/commits?*", (route) =>
    route.fulfill({
      json: wireFixture({ articles: [], total: 0, unreadTotal: 0 }),
    }),
  );
  await page.goto("/reading?workspace=ws&article=1");
  const proceed = page.getByRole("button", { name: "Далее →", exact: true });
  await expect(proceed).toBeEnabled();
  await proceed.click();
  await expect(heading(page)).toHaveText("Article 2");
  expect(f.calls.ordinary).toBe(0);
  expect(f.calls.complete).toBe(0);
});

for (const width of [1440, 390]) {
  test(`archive commit opens without AI requests; explicit TLDR has stable pending and no duplicate activation at ${width}px`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 900 });
    const f = await fixture(page);
    f.articles[0].url = "https://github.com/acme/archive/commit/a";
    let generated = false;
    let requests = 0;
    let release!: () => void;
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    await page.route("**/api/articles/1/reading/commits?*", (route) =>
      route.fulfill({
        json: wireFixture({
          articles: [f.articles[0]],
          total: 1,
          unreadTotal: 1,
          olderCursor: null,
          newerCursor: null,
        }),
      }),
    );
    await page.route("**/api/articles/1/chats?*", (route) =>
      route.fulfill({ json: generated ? [f.chat("1")] : [] }),
    );
    await page.route("**/api/articles/1/chat", async (route) => {
      requests++;
      await held;
      generated = true;
      await route.fulfill({ json: f.chat("1") });
    });
    let termRequests = 0;
    await page.route("**/api/articles/1/definitions*", (route) => {
      if (route.request().method() === "POST") termRequests++;
      return route.fulfill({
        json: {
          channel: {
            channel: "reading_data_news",
            configured: false,
            indexReady: true,
            revision: 1,
            posts: 0,
            definitions: 0,
            unindexed: 0,
            pending: 0,
            conflicts: 0,
            historyIncomplete: false,
            coverageNote: "",
            syncPending: false,
            generationAllowed: true,
          },
          known: [],
        },
      });
    });
    await page.goto("/reading?workspace=ws&article=1");
    const button = page.getByRole("button", {
      name: "Сделать TL;DR: Article 1",
      exact: true,
    });
    await expect(button).toBeVisible();
    await expect(
      page
        .getByLabel("TL;DR", { exact: true })
        .getByText("Готового TL;DR нет. Запустите пересказ явно."),
    ).toBeVisible();
    expect(requests).toBe(0);
    expect(termRequests).toBe(0);
    const footer = page.getByRole("button", {
      name: "Все прочитаны →",
      exact: true,
    });
    const before = await button.boundingBox();
    const footerBefore = await footer.boundingBox();
    await button.click();
    await expect(button).toHaveAttribute("aria-busy", "true");
    await expect(button).toBeDisabled();
    await button.evaluate((value: HTMLButtonElement) => value.click());
    await expect.poll(() => requests).toBe(1);
    expect(await button.boundingBox()).toEqual(before);
    expect(await footer.boundingBox()).toEqual(footerBefore);
    release();
    await expect(page.locator(".commit-group__paragraph")).toContainText(
      "Prepared summary for 1",
    );
    expect(await button.boundingBox()).toEqual(before);
    expect(await footer.boundingBox()).toEqual(footerBefore);
    expect(termRequests).toBe(0);
  });
}

for (const width of [1440, 390]) {
  test(`session wizard and timed smart/random reader preserve targets and ratings at ${width}px`, async ({
    page,
  }, testInfo) => {
    await page.setViewportSize({ width, height: 900 });
    await page.clock.install();
    const f = await fixture(page);
    let randomCalls = 0;
    await page.route("**/api/ai/smart-feed?*", (route) =>
      route.fulfill({
        json: {
          profile: null,
          total: 3,
          scored: 3,
          failed: 0,
          nextCursor: null,
          articles: [...f.articles]
            .reverse()
            .filter((a) => !a.read)
            .map((a) => ({
              id: a.id,
              title: a.title,
              excerpt: a.excerpt,
              prediction: null,
              error: null,
            })),
        },
      }),
    );
    await page.route("**/api/ai/random-feed?*", (route) => {
      randomCalls++;
      expect(new URL(route.request().url()).searchParams.get("seed")).toMatch(
        /^[a-f0-9-]{36}$/,
      );
      return route.fulfill({
        json: {
          profile: null,
          total: 3,
          scored: 3,
          failed: 0,
          nextCursor: null,
          articles: f.articles
            .filter((a) => !a.read)
            .map((a) => ({
              id: a.id,
              title: a.title,
              excerpt: a.excerpt,
              prediction: null,
              error: null,
            })),
        },
      });
    });
    await page.goto("/");
    await page
      .getByRole("button", { name: "Начать чтение", exact: true })
      .click();
    const dialog = page.getByRole("dialog", { name: "Сессия чтения" });
    const smart = dialog.getByLabel("Умная лента · минуты"),
      random = dialog.getByLabel("Случайная лента · минуты");
    await expect(smart).toHaveValue("45");
    await expect(random).toHaveValue("15");
    const start = dialog.getByRole("button", {
      name: "Начать чтение",
      exact: true,
    });
    const startBox = await start.boundingBox();
    await smart.fill("0");
    await random.fill("0");
    await start.click();
    await expect(dialog.getByRole("status")).toContainText("хотя бы одному");
    expect(await start.boundingBox()).toEqual(startBox);
    await smart.fill("1");
    await random.fill("1");
    await start.click();
    await expect(heading(page)).toHaveText("Article 3");
    const clock = page.getByLabel("Этап чтения");
    await expect(clock).toContainText("Умная лента");
    const returning = page.getByRole("button", {
      name: "К текущей новости",
      exact: true,
    });
    const returnBox = await returning.boundingBox();
    const background = await page.context().newPage();
    await background.bringToFront();
    await page.clock.fastForward(10000);
    await page.bringToFront();
    await background.close();
    await expect(clock.locator(".reading-clock__digits")).toHaveText("0:50");
    const body = page.locator(".focused-reading__article .reader-body");
    await body.evaluate((el) => {
      el.scrollTop = 180;
    });
    const scroll = await body.evaluate((el) => el.scrollTop);
    await page.getByRole("link", { name: "Reader home", exact: true }).click();
    await page.clock.fastForward(5000);
    await expect(clock.locator(".reading-clock__digits")).toHaveText("0:45");
    expect(await returning.boundingBox()).toEqual(returnBox);
    let releaseReturn!: () => void,
      returnCalls = 0;
    const returnGate = new Promise<void>((resolve) => {
      releaseReturn = resolve;
    });
    await page.route("**/api/articles/3?*", async (route) => {
      returnCalls++;
      await returnGate;
      await route.fallback();
    });
    await returning.click();
    await expect(returning).toBeDisabled();
    await expect(returning).toHaveAttribute("aria-busy", "true");
    expect(await returning.boundingBox()).toEqual(returnBox);
    await returning.evaluate((el) => {
      (el as HTMLButtonElement).click();
      (el as HTMLButtonElement).click();
    });
    await expect.poll(() => returnCalls).toBe(1);
    releaseReturn();
    await expect(heading(page)).toHaveText("Article 3");
    await expect(returning).toHaveAttribute("aria-busy", "false");
    expect(await body.evaluate((el) => el.scrollTop)).toBe(scroll);
    expect(f.calls.generation).toBe(0);
    await page.reload();
    await expect(heading(page)).toHaveText("Article 3");
    await expect(clock.locator(".reading-clock__digits")).toHaveText("0:45");
    await page.screenshot({ path: testInfo.outputPath("unified-timer.png") });
    const footer = await next(page).boundingBox(),
      pause = page.getByRole("button", { name: "Пауза таймера", exact: true }),
      pauseBox = await pause.boundingBox();
    await page.clock.fastForward(61000);
    await expect(clock).toContainText("смена после статьи");
    await expect(heading(page)).toHaveText("Article 3");
    expect(await next(page).boundingBox()).toEqual(footer);
    expect(await pause.boundingBox()).toEqual(pauseBox);
    await rate(page, 4).click();
    await next(page).click();
    const feedback = page.getByRole("dialog", { name: "Почему 4 из 10?" });
    await feedback.getByRole("button", { name: "Rate 9 out of 10" }).click();
    await save(page).click();
    await expect(heading(page)).toHaveText("Article 1");
    await expect(clock).toContainText("Случайная лента");
    expect(randomCalls).toBe(1);
    expect(f.states[2].rating).toBe(9);
    expect(await next(page).boundingBox()).toEqual(footer);
    await page
      .getByRole("button", { name: "Пауза таймера", exact: true })
      .click();
    const paused = await clock.textContent();
    await page.clock.fastForward(30000);
    expect(await clock.textContent()).toBe(paused);
    await page.reload();
    await expect(heading(page)).toHaveText("Article 1");
    await expect(clock).toContainText("Случайная лента");
    await expect(
      page.getByRole("button", { name: "Продолжить таймер", exact: true }),
    ).toHaveAttribute("aria-pressed", "true");
    await page
      .getByRole("button", { name: "Продолжить таймер", exact: true })
      .click();
    await page.clock.fastForward(61000);
    await rate(page, 7).click();
    await next(page).click();
    await save(page).click();
    await expect(
      page.getByRole("heading", { name: "Сессия завершена", exact: true }),
    ).toBeVisible();
    expect(f.articles[1].read).toBe(false);
    expect(f.calls.generation).toBe(0);
  });
  test(`bulk mark requires confirmation and keeps pending/error targets stable at ${width}px`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 900 });
    const f = await fixture(page);
    let calls = 0,
      fail = true,
      release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    await page.route(
      "**/api/workspaces/ws/articles/mark-all-read",
      async (route) => {
        calls++;
        if (fail) {
          await gate;
          return route.fulfill({
            status: 503,
            json: { message: "Bulk write failed" },
          });
        }
        for (const a of f.articles) a.read = true;
        return route.fulfill({ status: 204 });
      },
    );
    await page.goto("/reader");
    const open = page.getByRole("button", {
      name: "Mark all read",
      exact: true,
    });
    await open.click();
    expect(calls).toBe(0);
    let dialog = page.getByRole("dialog", {
      name: "Отметить все статьи прочитанными?",
    });
    await dialog.getByRole("button", { name: "Отмена", exact: true }).click();
    expect(calls).toBe(0);
    await expect(open).toBeFocused();
    await open.click();
    dialog = page.getByRole("dialog", {
      name: "Отметить все статьи прочитанными?",
    });
    const confirm = dialog.getByRole("button", {
        name: "Да, все прочитаны",
        exact: true,
      }),
      box = await confirm.boundingBox(),
      cancel = dialog.getByRole("button", { name: "Отмена", exact: true }),
      cancelBox = await cancel.boundingBox();
    await confirm.click();
    await expect(confirm).toBeDisabled();
    await expect(confirm).toHaveAttribute("aria-busy", "true");
    await expect(cancel).toBeDisabled();
    expect(await confirm.boundingBox()).toEqual(box);
    expect(await cancel.boundingBox()).toEqual(cancelBox);
    await expect.poll(() => calls).toBe(1);
    release();
    await expect(dialog.getByRole("status")).toContainText("Не удалось");
    expect(await confirm.boundingBox()).toEqual(box);
    expect(await cancel.boundingBox()).toEqual(cancelBox);
    fail = false;
    await confirm.click();
    await expect(dialog).toHaveCount(0);
    expect(calls).toBe(2);
    expect(f.articles.every((a) => a.read)).toBe(true);
  });
}

test("queued automatic AI work is promoted only by explicit reader buttons with stable controls", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const f = await fixture(page);
  const queued = { ...f.chat("1"), status: "queued", messages: [] };
  const terms = {
    channel: {
      channel: "reading_data_news",
      configured: false,
      indexReady: true,
      revision: 1,
      posts: 0,
      definitions: 0,
      unindexed: 0,
      pending: 0,
      conflicts: 0,
      historyIncomplete: false,
      coverageNote: "",
      syncPending: false,
      generationAllowed: true,
    },
    known: [],
    job: {
      id: "terms-1",
      articleId: "1",
      workspaceId: "ws",
      model: "fixture",
      promptVersion: "fixture",
      status: "queued",
      usage: null,
    },
  };
  let summaries = 0,
    termRequests = 0;
  let summaryRelease!: () => void, termsRelease!: () => void;
  const summaryHeld = new Promise<void>((resolve) => {
    summaryRelease = resolve;
  });
  const termsHeld = new Promise<void>((resolve) => {
    termsRelease = resolve;
  });
  await page.route("**/api/articles/1/chats?*", (route) =>
    route.fulfill({ json: [queued] }),
  );
  await page.route("**/api/ai/chats/chat-1/changes*", (route) =>
    route.fulfill({ json: { revision: "1", chat: queued } }),
  );
  await page.route("**/api/articles/1/chat", async (route) => {
    summaries++;
    await summaryHeld;
    return route.fulfill({ json: f.chat("1") });
  });
  await page.route("**/api/articles/1/definitions*", async (route) => {
    if (route.request().method() === "POST") {
      termRequests++;
      expect(route.request().postDataJSON().regenerate).toBe(false);
      await termsHeld;
    }
    return route.fulfill({ json: terms });
  });
  await page.goto("/reading?workspace=ws&article=1");
  const summary = page.getByRole("button", { name: "Summarize", exact: true });
  const extract = page.getByRole("button", { name: "Terms", exact: true });
  await expect(summary).toBeEnabled();
  await expect(extract).toBeEnabled();
  await expect(page.locator(".glossary-panel--embedded")).toBeVisible();
  expect(summaries).toBe(0);
  expect(termRequests).toBe(0);
  const summaryBox = await summary.boundingBox();
  const termsBox = await extract.boundingBox();
  await summary.click();
  await expect.poll(() => summaries).toBe(1);
  await expect(summary).toHaveAttribute("aria-busy", "true");
  await expect(summary).toBeDisabled();
  await summary.evaluate((button) => {
    if (!(button instanceof HTMLButtonElement))
      throw new Error("Expected a native button");
    button.click();
    button.click();
  });
  expect(summaries).toBe(1);
  expect(await summary.boundingBox()).toEqual(summaryBox);
  expect(await extract.boundingBox()).toEqual(termsBox);
  summaryRelease();
  await expect(summary).toBeEnabled();
  await extract.click();
  await expect.poll(() => termRequests).toBe(1);
  await expect(extract).toHaveAttribute("aria-busy", "true");
  await expect(extract).toBeDisabled();
  await extract.evaluate((button) => {
    if (!(button instanceof HTMLButtonElement))
      throw new Error("Expected a native button");
    button.click();
    button.click();
  });
  expect(termRequests).toBe(1);
  expect(await summary.boundingBox()).toEqual(summaryBox);
  expect(await extract.boundingBox()).toEqual(termsBox);
  termsRelease();
  await expect(extract).toBeEnabled();
  expect(summaries).toBe(1);
  expect(termRequests).toBe(1);
});
