import { test, expect } from "@playwright/test";
import type { ArticleChat } from "../src/api/ai";
const chat = (articleId: string): ArticleChat => ({
  id: `chat-${articleId}`,
  articleId,
  workspaceId: "ws",
  title: `Prepared ${articleId}`,
  sourceUrl: "https://example.test",
  createdAt: "2026-09-29T12:00:00Z",
  model: "fixture",
  promptVersion: "fixture",
  status: "completed",
  providerCalls: [],
  messages: [
    {
      id: "summary",
      role: "assistant",
      purpose: "summary",
      phase: "verifying",
      status: "complete",
      content: `Prepared summary for ${articleId}`,
      createdAt: "2026-09-29T12:00:00Z",
    },
  ],
});
test("article selection automatically opens its saved chat after closing the previous one, without generating again or shifting controls", async ({
  page,
}) => {
  await page.route("**/api/articles?*", async (route) => {
    const response = await route.fetch();
    const body = await response.json();
    body.articles.forEach((article: { fullText: string }) => {
      article.fullText = "ready";
    });
    await route.fulfill({ json: body });
  });
  await page.route(/\/api\/articles\/2(?:\?|$)/, async (route) => {
    const response = await route.fetch();
    await route.fulfill({
      json: { ...(await response.json()), fullText: "ready" },
    });
  });
  let mutations = 0;
  await page.route("**/api/ai/profile", (route) =>
    route.fulfill({
      json: {
        models: { summary: "deepseek-flash", verification: "deepseek-flash" },
        configured: true,
        enabled: true,
      },
    }),
  );
  await page.route("**/api/articles/*/chats?*", (route) =>
    route.fulfill({
      json: [chat(new URL(route.request().url()).pathname.split("/")[3])],
    }),
  );
  await page.route("**/api/articles/*/chat", (route) => {
    mutations++;
    return route.fulfill({ json: chat("unexpected") });
  });
  await page.goto("/reader");
  const list = page.locator(".article-list");
  const chatPanel = page.getByRole("dialog", { name: /Article chat/ });
  await expect(
    page.getByText("Prepared summary for 1", { exact: true }),
  ).toBeVisible();
  const before = await list.boundingBox();
  await page.getByRole("button", { name: "Close chat" }).click();
  await page
    .getByRole("heading", {
      name: "The durable queue is the product",
      exact: true,
    })
    .click();
  await expect(
    page.getByText("Prepared summary for 2", { exact: true }),
  ).toBeVisible();
  expect(await list.boundingBox()).toEqual(before);
  expect(mutations).toBe(0);
  await expect(page.getByRole("button", { name: "New summary" })).toHaveCount(
    0,
  );
});

test("spending graph separates modes and retains all settings targets while refreshing", async ({
  page,
}) => {
  const profile = {
    models: { summary: "deepseek-flash", verification: "deepseek-flash" },
    configured: true,
    enabled: true,
    spending: {
      dailyLimitUsd: "3",
      today: "2026-09-29",
      resetsAt: "2026-09-29T21:00:00Z",
      spentUsd: "0.72",
      reservedUsd: "0.08",
      remainingUsd: "2.20",
      days: [
        {
          day: "2026-09-29",
          mode: "summary",
          spentUsd: "0.2",
          reservedUsd: "0",
        },
        {
          day: "2026-09-29",
          mode: "verification",
          spentUsd: "0.4",
          reservedUsd: "0.08",
        },
        { day: "2026-09-29", mode: "chat", spentUsd: "0.12", reservedUsd: "0" },
        {
          day: "2026-09-28",
          mode: "translation",
          spentUsd: "0.9",
          reservedUsd: "0",
        },
        { day: "2026-09-28", mode: "terms", spentUsd: "0.3", reservedUsd: "0" },
      ],
    },
  };
  let hold = false,
    release!: () => void;
  await page.route(/\/api\/ai\/(profile|balance)$/, async (route) => {
    if (hold) await new Promise<void>((resolve) => (release = resolve));
    await route.fulfill({ json: profile });
  });
  await page.goto("/reader");
  await page.getByRole("button", { name: "Account menu" }).click();
  await page.getByRole("menuitem", { name: "Profile", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Profile", exact: true });
  const refresh = dialog.getByRole("button", { name: "Refresh balance" });
  await expect(refresh).toBeEnabled();
  const plot = dialog.getByRole("region", { name: "Daily DeepSeek spending" });
  await plot.scrollIntoViewIfNeeded();
  const before = {
    plot: await plot.boundingBox(),
    refresh: await refresh.boundingBox(),
  };
  hold = true;
  // Keep the initiating control visible while measuring the fixed plot height.
  await refresh.scrollIntoViewIfNeeded();
  const controls = await refresh.boundingBox();
  await refresh.click();
  await expect(refresh).toBeDisabled();
  expect(await refresh.boundingBox()).toEqual(controls);
  profile.spending.spentUsd = "0.82";
  profile.spending.remainingUsd = "2.10";
  await expect.poll(() => typeof release).toBe("function");
  release();
  await expect(refresh).toBeEnabled();
  expect(await refresh.boundingBox()).toEqual(controls);
  expect((await plot.boundingBox())?.height).toEqual(before.plot?.height);
  await plot.scrollIntoViewIfNeeded();
  const last = plot.getByRole("img").last();
  await last.hover();
  await expect(last.locator(".ai-spending__tooltip")).toBeVisible();
  await expect(last.locator(".ai-spending__tooltip")).toContainText(
    "Fact-check: $0.4",
  );
  await dialog.screenshot({
    path: test.info().outputPath("deepseek-spending.png"),
  });
});

for (const state of ["failed", "pending"] as const) {
  test(`articles with ${state} full text neither open chat nor request summaries`, async ({
    page,
  }) => {
    let unavailableRequests = 0;
    await page.route("**/api/ai/profile", (route) =>
      route.fulfill({
        json: {
          models: { summary: "deepseek-flash", verification: "deepseek-flash" },
          configured: true,
          enabled: true,
        },
      }),
    );
    await page.route("**/api/articles?*", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      body.articles[1].fullText = state;
      await route.fulfill({ json: body });
    });
    await page.route(/\/api\/articles\/2(?:\?|$)/, async (route) => {
      const response = await route.fetch();
      await route.fulfill({
        json: { ...(await response.json()), fullText: state },
      });
    });
    await page.route("**/api/articles/*/chats?*", (route) => {
      const id = new URL(route.request().url()).pathname.split("/")[3];
      if (id === "2") unavailableRequests++;
      return route.fulfill({ json: [chat(id)] });
    });
    await page.route("**/api/articles/2/chat", (route) => {
      unavailableRequests++;
      return route.fulfill({ json: chat("2") });
    });
    await page.goto("/reader");
    await expect(
      page.getByRole("dialog", { name: /Article chat/ }),
    ).toBeVisible();
    const list = page.locator(".article-list");
    const before = await list.boundingBox();
    await page
      .getByRole("heading", {
        name: "The durable queue is the product",
        exact: true,
      })
      .click();
    await expect(
      page.getByRole("button", { name: "Summarize", exact: true }),
    ).toBeDisabled();
    await expect(
      page.getByRole("dialog", { name: /Article chat/ }),
    ).toBeHidden();
    expect(await list.boundingBox()).toEqual(before);
    expect(unavailableRequests).toBe(0);
    await page
      .getByRole("heading", {
        name: "Async Rust without the hidden machinery",
        exact: true,
      })
      .click();
    await expect(
      page.getByRole("dialog", { name: /Article chat/ }),
    ).toBeVisible();
    expect(unavailableRequests).toBe(0);
  });
}
