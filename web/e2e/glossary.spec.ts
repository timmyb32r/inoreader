import { wireFixture } from "./wire-fixtures.mjs";
import { expect, test } from "@playwright/test";
import type { ChannelStatus, DefinitionsView } from "../src/api/glossary";

test("terms show immediate feedback, deduplicate, preserve targets and split original definitions", async ({
  page,
}) => {
  const channel: ChannelStatus = {
    channel: "reading_data_news",
    configured: false,
    botUsername: null,
    indexReady: true,
    revision: 1,
    posts: 1529,
    definitions: 400,
    unindexed: 0,
    pending: 0,
    conflicts: 0,
    lastPoll: null,
    lastHistory: null,
    pollError: null,
    historyError: null,
    historyIncomplete: true,
    coverageNote: "Публичная история может быть неполной",
    syncPending: false,
    generationAllowed: true,
  };
  const article = {
    id: "a",
    title:
      "TapTalk | 圆桌实录：澳门综合度假村敏捷转型之旅，MongoDB + TapData 赋能酒店业卓越实践 ".repeat(
        5,
      ),
    url: "https://example.com/a",
    source: "Example",
    subscriptionIds: ["source"],
    excerpt: "Intro",
    body: ["CDC and Kafka."],
    fullText: "ready",
    savedAt: "2026-09-26T12:00:00Z",
    read: false,
    later: false,
  };
  const articlePage = { articles: [article], total: 1, unreadTotal: 1 };
  let mutations = 0,
    complete = false,
    failedPolls = 2,
    configurationCalls = 0,
    releaseConfiguration = false;
  let job: DefinitionsView["job"] = null;
  await page.route("**/api/**", async (route) => {
    const req = route.request(),
      path = new URL(req.url()).pathname;
    if (path === "/api/bootstrap")
      return route.fulfill({
        json: wireFixture({
          account: { id: "owner", displayName: "Author", initials: "AU" },
          workspaces: [{ id: "ws", name: "Personal", archived: false }],
          activeWorkspaceId: "ws",
          subscriptions: [],
          articlePage,
        }),
      });
    if (path === "/api/ai/profile")
      return route.fulfill({
        json: wireFixture({
          models: { summary: "deepseek-flash", verification: "deepseek-flash" },
          enabled: true,
          configured: true,
        }),
      });
    if (path === "/api/articles")
      return route.fulfill({ json: wireFixture(articlePage) });
    if (path === "/api/articles/a" || path === "/api/articles/a/state")
      return route.fulfill({ json: wireFixture(article) });
    if (path === "/api/glossary/channel") {
      if (req.method() === "PUT") {
        configurationCalls++;
        while (!releaseConfiguration)
          await new Promise((r) => setTimeout(r, 20));
      }
      return route.fulfill({ json: wireFixture(channel) });
    }
    if (path === "/api/articles/a/definitions") {
      if (req.method() === "GET" && job && failedPolls > 0) {
        failedPolls--;
        return route.fulfill({
          status: 503,
          json: wireFixture({
            error: { code: "temporary", message: "Temporary network failure" },
          }),
        });
      }
      if (req.method() === "POST") {
        mutations++;
        job = {
          id: "job",
          workspaceId: "ws",
          articleId: "a",
          model: "flash",
          promptVersion: "v1",
          status: "queued",
        };
      }
      if (complete && job)
        job = {
          ...job,
          status: "completed",
          result: {
            entities: [
              {
                name: "CDC",
                kind: "abbreviation",
                explanation: "Новое сгенерированное определение CDC",
                insufficientContext: false,
              },
              {
                name: "Kafka",
                kind: "product",
                explanation: "Платформа для потоков событий.",
                insufficientContext: false,
              },
            ],
          },
        };
      return route.fulfill({
        json: wireFixture({
          job,
          channel,
          known: complete
            ? [
                {
                  definition: {
                    term: "CDC",
                    position: 0,
                    paragraph: {
                      text: "CDC — точный абзац из канала.",
                      marks: [{ start: 0, end: 3, style: { kind: "bold" } }],
                    },
                  },
                  permalink: "https://t.me/reading_data_news/42",
                  publishedAt: null,
                  stale: false,
                },
              ]
            : [],
        }),
      });
    }
    return route.fulfill({ json: wireFixture([]) });
  });
  await page.goto("/reader");
  await page.getByRole("button", { name: "Close chat" }).click();
  const trigger = page.getByRole("button", { name: "Terms", exact: true });
  const before = await trigger.boundingBox();
  await trigger.click();
  const dialog = page.getByRole("dialog", { name: "Термины статьи" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole("status")).toHaveAttribute("aria-busy", "true");
  await expect(trigger).toBeDisabled();
  expect(mutations).toBe(1);
  const close = dialog.getByRole("button", { name: "Закрыть термины" }),
    closeBefore = await close.boundingBox();
  const copy = dialog.getByRole("button", {
      name: "Скопировать все новые определения",
    }),
    copyBefore = await copy.boundingBox();
  complete = true;
  await expect(
    dialog.getByText("Платформа для потоков событий.", { exact: false }),
  ).toBeVisible();
  await expect(
    dialog.getByText("Новое сгенерированное определение CDC"),
  ).toHaveCount(0);
  await expect(
    dialog.getByRole("link", { name: "Пост в канале ↗" }),
  ).toHaveAttribute("href", "https://t.me/reading_data_news/42");
  expect(await trigger.boundingBox()).toEqual(before);
  expect(await close.boundingBox()).toEqual(closeBefore);
  expect(await copy.boundingBox()).toEqual(copyBefore);
  const copied = await dialog
    .locator(".glossary-entry strong")
    .allTextContents();
  expect(copied).toEqual(["Kafka", "CDC"]);
  for (const selector of [
    ".glossary-panel",
    ".glossary-content",
    ".glossary-panel header",
  ]) {
    expect(
      await page
        .locator(selector)
        .evaluate((el) => el.scrollWidth <= el.clientWidth + 1),
    ).toBe(true);
  }
  await close.click();
  await expect(trigger).toBeFocused();
  await trigger.click();
  await expect(dialog).toBeVisible();
  expect(mutations).toBe(1);
  await expect(
    dialog.getByText("Платформа для потоков событий.", { exact: false }),
  ).toBeVisible();
  await page.screenshot({
    path: "test-results/glossary-panel.png",
    fullPage: true,
  });
  await dialog.getByRole("button", { name: "Обновить определения" }).click();
  await expect.poll(() => mutations).toBe(2);
  await close.click();
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  const token = page.getByLabel("Токен отдельного Telegram-бота");
  await token.fill("123:fake-test-token");
  expect(await token.getAttribute("autocomplete")).toBe("none");
  const connect = page.getByRole("button", {
    name: "Подключить бота",
    exact: true,
  });
  await connect.scrollIntoViewIfNeeded();
  const connectBefore = await connect.boundingBox(),
    tokenBefore = await token.boundingBox();
  await connect.dblclick();
  await expect(connect).toBeDisabled();
  await expect(connect).toHaveAttribute("aria-busy", "true");
  expect(configurationCalls).toBe(1);
  expect(await token.boundingBox()).toEqual(tokenBefore);
  releaseConfiguration = true;
  await expect(token).toHaveValue("");
  expect(await connect.boundingBox()).toEqual(connectBefore);
  expect(await token.boundingBox()).toEqual(tokenBefore);
});
