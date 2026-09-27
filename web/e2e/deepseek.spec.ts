import { expect, test, type Page } from "@playwright/test";
import type { AiProfile, ArticleChat } from "../src/api/ai";

const exampleChat = (articleId = "a", id = "chat-a"): ArticleChat => ({ id, articleId, workspaceId: "ws", title: `Article ${articleId.toUpperCase()}`, sourceUrl: "https://example.test/article", createdAt: "2026-09-26T12:00:00Z", model: "mock-deepseek", promptVersion: "candidate-1", status: "completed", providerCalls: [], messages: [{ id: "assistant", role: "assistant", purpose: "summary", phase: "verifying", status: "complete", content: "# Original title\n\n**Kafka** stores events.\n\n> Exact statement.\n\n<script>window.infected = true</script>\n\n[Source](https://example.test/article)\n\n![Tracking](https://tracking.example.test/pixel)", createdAt: "2026-09-26T12:00:00Z" }] });

async function fixture(page: Page, initialProfile: AiProfile = { configured: true, enabled: true }) {
  const state = { profile: initialProfile, versions: [] as ArticleChat[], mutations: [] as { path: string; body: Record<string, unknown> }[], reads: [] as string[], hold: false, status: "generating" as ArticleChat["status"] };
  const articles = ["a", "b"].map(id => ({ id, title: `Article ${id.toUpperCase()}`, url: `https://example.test/${id}`, source: "Example", subscriptionIds: ["source"], excerpt: "Article excerpt", body: ["Exact statement.", "Full article body."], fullText: "ready", age: "2026-09-26T12:00:00Z", read: false, later: false }));
  const articlePage = { articles, total: 2, unreadTotal: 2 };
  await page.route("**/api/**", async route => {
    const request = route.request(), url = new URL(request.url()), path = url.pathname;
    const body = request.postData() ? request.postDataJSON() : {};
    if (request.method() !== "GET") state.mutations.push({ path, body }); else state.reads.push(path);
    if (path === "/api/bootstrap") return route.fulfill({ json: { account: { id: "owner", displayName: "Author", initials: "AU" }, workspaces: [{ id: "ws", name: "Personal", archived: false }], activeWorkspaceId: "ws", subscriptions: [], articlePage } });
    if (path === "/api/articles") return route.fulfill({ json: articlePage });
    const article = articles.find(item => path === `/api/articles/${item.id}` || path === `/api/articles/${item.id}/state`);
    if (article) return route.fulfill({ json: { ...article, ...body } });
    if (path === "/api/ai/profile" || path === "/api/ai/balance") {
      while (state.hold && request.method() === "PUT") await new Promise(resolve => setTimeout(resolve, 25));
      if (request.method() === "PUT") state.profile = { configured: true, enabled: true, balance: { available: true, updatedAt: "2026-09-26T13:00:00Z", balances: [{ currency: "USD", total: "8.123456", granted: "0", toppedUp: "8.123456" }] } };
      if (request.method() === "DELETE") state.profile = { configured: false, enabled: false };
      return route.fulfill({ json: state.profile });
    }
    if (path.endsWith("/chats")) return route.fulfill({ json: state.versions.filter(chat => path.includes(`/articles/${chat.articleId}/`)) });
    if (path.endsWith("/chat")) {
      const id = path.split("/")[3], chat = { ...exampleChat(id, `chat-${id}-${state.versions.length}`), status: state.status, messages: [] };
      state.versions.unshift(chat);
      return route.fulfill({ json: chat });
    }
    if (path.startsWith("/api/ai/chats/")) {
      const id = path.split("/")[4], chat = state.versions.find(item => item.id === id)!;
      if (path.endsWith("/stop")) { chat.status = "cancelled"; return route.fulfill({ json: chat }); }
      if (path.endsWith("/messages") || path.endsWith("/retry")) { chat.status = "generating"; return route.fulfill({ json: chat }); }
      chat.status = state.status;
      if (chat.status === "completed") chat.messages = exampleChat(chat.articleId, chat.id).messages;
      return route.fulfill({ json: chat });
    }
    return route.fulfill({ json: [] });
  });
  return state;
}

test("chat drag, response, collapse and viewport resize keep reader/composer targets stable", async ({ page }) => {
  const state = await fixture(page);
  await page.goto("/reader");
  const summarize = page.getByRole("button", { name: "Summarize", exact: true });
  await expect(summarize).toBeVisible();
  expect(state.mutations.filter(call => call.path.includes("/chat"))).toHaveLength(0);
  const toolbar = page.locator(".reader-toolbar"), before = await toolbar.boundingBox();
  await summarize.dblclick();
  const chat = page.getByRole("dialog", { name: "Article chat: Article A", exact: true });
  await expect(chat).toBeVisible();
  await expect(chat.getByRole("status")).toHaveAttribute("aria-busy", "true");
  await expect.poll(() => state.mutations.filter(call => call.path.endsWith("/chat")).length).toBe(1);
  expect(await toolbar.boundingBox()).toEqual(before);
  const handle = chat.getByRole("button", { name: "Move chat with arrow keys or drag" });
  const handleBox = (await handle.boundingBox())!, chatBefore = (await chat.boundingBox())!;
  await page.mouse.move(handleBox.x + 40, handleBox.y + 25);
  await page.mouse.down(); await page.mouse.move(handleBox.x - 120, handleBox.y - 10, { steps: 8 }); await page.mouse.up();
  expect((await chat.boundingBox())!.x).toBeLessThan(chatBefore.x);
  expect(await toolbar.boundingBox()).toEqual(before);
  const composer = chat.getByLabel("Message DeepSeek"), composerBefore = await composer.boundingBox();
  state.status = "completed";
  await expect(chat.getByText("Kafka", { exact: true })).toBeVisible();
  expect(await composer.boundingBox()).toEqual(composerBefore);
  expect(await toolbar.boundingBox()).toEqual(before);
  expect(await page.evaluate(() => (window as Window & { infected?: boolean }).infected)).toBeUndefined();
  await expect(chat.locator("script, img")).toHaveCount(0);
  await expect(chat.getByRole("link", { name: "Source" })).toHaveAttribute("rel", "noopener noreferrer");
  await page.screenshot({ path: test.info().outputPath("article-chat.png") });
  await composer.fill("Keep my draft");
  await chat.getByRole("button", { name: "Minimize chat" }).click();
  await expect(composer).not.toBeVisible();
  await chat.getByRole("button", { name: "Expand chat" }).click();
  await expect(composer).toHaveValue("Keep my draft");
  await page.setViewportSize({ width: 390, height: 720 });
  const mobile = (await chat.boundingBox())!;
  expect(mobile.x).toBeGreaterThanOrEqual(0); expect(mobile.x + mobile.width).toBeLessThanOrEqual(390);
  expect(mobile.y).toBeGreaterThanOrEqual(0); expect(mobile.y + mobile.height).toBeLessThanOrEqual(720);
  await page.setViewportSize({ width: 640, height: 300 });
  const compact = (await chat.boundingBox())!, compactComposer = (await composer.boundingBox())!;
  const compactSend = (await chat.getByRole("button", { name: "Send", exact: true }).boundingBox())!;
  expect(compact.y).toBeGreaterThanOrEqual(0); expect(compact.y + compact.height).toBeLessThanOrEqual(300);
  expect(compactComposer.y).toBeGreaterThanOrEqual(compact.y + 60);
  expect(compactSend.y + compactSend.height).toBeLessThanOrEqual(compact.y + compact.height);
  expect(await chat.getByRole("button", { name: "Send", exact: true }).evaluate(element => {
    const rect = element.getBoundingClientRect();
    return element.contains(document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2));
  })).toBe(true);
  await chat.locator(".ai-chat__conversation").evaluate(element => { element.scrollTop = element.scrollHeight; });
  await expect(chat.getByRole("button", { name: "Profile", exact: true })).toBeInViewport();
  expect(await composer.boundingBox()).toEqual(compactComposer);
  await chat.getByRole("button", { name: "Close chat" }).click();
  await expect(chat).not.toBeVisible();
});

test("profile key saving and balance updates preserve controls and never store the secret in the browser", async ({ page }) => {
  const state = await fixture(page, { configured: false, enabled: false });
  await page.goto("/reader");
  await page.getByRole("button", { name: "Summarize", exact: true }).click();
  const chat = page.getByRole("dialog", { name: "Article chat: Article A" });
  await expect(chat.getByRole("status")).toContainText("Add and validate");
  expect(state.mutations.filter(call => call.path.endsWith("/chat"))).toHaveLength(0);
  await chat.getByRole("button", { name: "Open DeepSeek profile" }).click();
  const profile = page.getByRole("dialog", { name: "Profile", exact: true });
  const field = profile.getByLabel("DeepSeek API key");
  await expect(field).toBeEnabled();
  await field.fill("test-ui-secret");
  await expect(field).toHaveAttribute("type", "password");
  await expect(field).toHaveAttribute("autocomplete", "none");
  const save = profile.getByRole("button", { name: "Save API key" }), balance = profile.getByRole("button", { name: "Refresh balance" });
  const saveBefore = await save.boundingBox(), balanceBefore = await balance.boundingBox();
  state.hold = true;
  await save.dblclick();
  await expect(profile.getByRole("button", { name: "Saving…" })).toBeDisabled();
  expect(await balance.boundingBox()).toEqual(balanceBefore);
  expect(state.mutations.filter(call => call.path === "/api/ai/profile")).toHaveLength(1);
  state.hold = false;
  await expect(profile.getByText("8.123456 USD")).toBeVisible();
  expect(await save.boundingBox()).toEqual(saveBefore);
  expect(await balance.boundingBox()).toEqual(balanceBefore);
  await expect(profile.getByLabel("Replace DeepSeek API key")).toHaveValue("");
  const storage = await page.evaluate(() => JSON.stringify({ local: { ...localStorage }, session: { ...sessionStorage } }));
  expect(storage).not.toContain("test-ui-secret");
});

test("chat stays bound to its article and reopens without generation; waiting full text can be stopped", async ({ page }) => {
  const state = await fixture(page);
  state.status = "waiting_content";
  await page.goto("/reader");
  await page.getByRole("button", { name: "Summarize", exact: true }).click();
  const chat = page.getByRole("dialog", { name: "Article chat: Article A", exact: true });
  await expect(chat.getByRole("status")).toContainText("Waiting for the full article");
  await page.getByRole("heading", { name: "Article B", level: 2 }).click();
  await expect(chat).toBeVisible();
  await expect(page.getByRole("heading", { name: "Article B", level: 1 })).toBeVisible();
  await chat.getByRole("button", { name: "Stop", exact: true }).click();
  await expect(chat.getByRole("status")).toContainText("Stopped");
  await expect(chat.getByRole("button", { name: "Retry", exact: true })).toBeEnabled();
  await chat.getByRole("button", { name: "Close chat" }).click();
  await page.getByRole("heading", { name: "Article A", level: 2 }).click();
  await page.getByRole("button", { name: "Summarize", exact: true }).click();
  await expect(chat.getByRole("status")).toContainText("Stopped");
  expect(state.mutations.filter(call => call.path.endsWith("/chat"))).toHaveLength(1);
  await page.reload();
  await page.getByRole("button", { name: "Summarize", exact: true }).click();
  await expect(chat.getByRole("status")).toContainText("Stopped");
  expect(state.mutations.filter(call => call.path.endsWith("/chat"))).toHaveLength(1);
});

test("lost message acknowledgements retain one operation and stable controls until explicit recovery", async ({ page }) => {
  const state = await fixture(page);
  state.status = "completed";
  state.versions.push(exampleChat());
  const operations: { operationId: string; content: string }[] = [];
  let release!: () => void;
  const held = new Promise<void>(resolve => { release = resolve; });
  await page.route("**/api/ai/chats/*/messages", async route => {
    operations.push(route.request().postDataJSON());
    if (operations.length === 1) { await held; return route.abort("connectionreset"); }
    return route.fulfill({ json: state.versions[0] });
  });
  await page.goto("/reader");
  await page.getByRole("button", { name: "Summarize", exact: true }).click();
  const chat = page.getByRole("dialog", { name: "Article chat: Article A", exact: true });
  const composer = chat.getByLabel("Message DeepSeek"), send = chat.getByRole("button", { name: "Send", exact: true });
  await composer.fill("Explain the mechanism");
  const before = { composer: await composer.boundingBox(), send: await send.boundingBox(), toolbar: await page.locator(".reader-toolbar").boundingBox() };
  await send.dblclick();
  await expect(send).toBeDisabled();
  await expect(send).toHaveAttribute("aria-busy", "true");
  expect(operations).toHaveLength(1);
  expect(await composer.boundingBox()).toEqual(before.composer);
  expect(await send.boundingBox()).toEqual(before.send);
  release();
  await expect(chat.getByRole("status")).toContainText("request outcome is unknown");
  await expect(send).toBeDisabled();
  await expect(chat.getByRole("button", { name: "New summary" })).toBeDisabled();
  await chat.getByRole("button", { name: "Reconnect" }).click();
  await expect(send).toBeDisabled();
  expect(await composer.boundingBox()).toEqual(before.composer);
  expect(await send.boundingBox()).toEqual(before.send);
  expect(await page.locator(".reader-toolbar").boundingBox()).toEqual(before.toolbar);
  await chat.getByRole("button", { name: "Retry", exact: true }).click();
  await expect(composer).toHaveValue("");
  expect(operations).toHaveLength(2);
  expect(operations[1]).toEqual(operations[0]);
  expect(await composer.boundingBox()).toEqual(before.composer);
  expect(await send.boundingBox()).toEqual(before.send);
});

test("closing Profile during key save still enables the account when saving completes", async ({ page }) => {
  const state = await fixture(page, { configured: false, enabled: false });
  await page.goto("/reader");
  await page.getByRole("button", { name: "Summarize", exact: true }).click();
  const chat = page.getByRole("dialog", { name: "Article chat: Article A" });
  await chat.getByRole("button", { name: "Open DeepSeek profile" }).click();
  const profile = page.getByRole("dialog", { name: "Profile", exact: true });
  const field = profile.getByLabel("DeepSeek API key");
  await expect(field).toBeEnabled();
  await field.fill("fixture-key");
  state.hold = true;
  await profile.getByRole("button", { name: "Save API key" }).click();
  await expect(profile.getByRole("button", { name: "Saving…" })).toBeDisabled();
  await profile.getByRole("button", { name: "Done", exact: true }).click();
  state.hold = false;
  await expect(chat.getByText("Ctrl / ⌘ + Enter to send · no web search")).toBeVisible();
  await chat.getByRole("button", { name: "Close chat" }).click();
  await page.getByRole("button", { name: "Summarize", exact: true }).click();
  await expect(chat.getByRole("status")).toContainText("DeepSeek is writing");
  expect(state.mutations.filter(call => call.path.endsWith("/chat"))).toHaveLength(1);
});

test("generation and verification keep hit targets fixed and only publish the final checked summary", async ({ page }) => {
  const state = await fixture(page);
  const current: ArticleChat = { ...exampleChat(), status: "generating", messages: [{ ...exampleChat().messages[0], phase: "generating", status: "streaming", content: "Unverified private draft" }], providerCalls: [{ id: "generation", assistantId: "assistant", phase: "generating", status: "started" }] };
  state.versions.push(current);
  await page.goto("/reader");
  await page.getByRole("button", { name: "Summarize", exact: true }).click();
  const chat = page.getByRole("dialog", { name: "Article chat: Article A", exact: true });
  await expect(chat.getByRole("status")).toContainText("Step 1 of 2");
  const stop = chat.getByRole("button", { name: "Stop", exact: true }), send = chat.getByRole("button", { name: "Send", exact: true }), composer = chat.getByLabel("Message DeepSeek"), toolbar = page.locator(".reader-toolbar");
  const before = { stop: await stop.boundingBox(), send: await send.boundingBox(), composer: await composer.boundingBox(), toolbar: await toolbar.boundingBox() };
  await expect(chat.getByText("Unverified private draft")).toHaveCount(0);
  await expect(chat.getByRole("button", { name: "Copy response" })).toBeDisabled();
  const usage = { promptTokens: 11000, completionTokens: 250, promptCacheHitTokens: 1000, promptCacheMissTokens: 10000, estimatedCostUsd: "0.010003" };
  current.messages[0].phase = "verifying";
  current.providerCalls = [{ ...current.providerCalls[0], status: "completed", usage }, { id: "verification", assistantId: "assistant", phase: "verifying", status: "started" }];
  state.status = "verifying";
  await expect(chat.getByRole("status")).toContainText("Step 2 of 2");
  await expect(chat.getByRole("status")).toHaveAttribute("aria-busy", "true");
  await expect(chat.getByLabel("Provider request costs")).toContainText("2 provider requests · Known estimate: $0.010003 · 1 cost unknown");
  await expect(chat.getByText("Unverified private draft")).toHaveCount(0);
  await expect(send).toBeDisabled();
  await expect(chat.getByRole("button", { name: "New summary" })).toBeDisabled();
  expect(await stop.boundingBox()).toEqual(before.stop);
  expect(await send.boundingBox()).toEqual(before.send);
  expect(await composer.boundingBox()).toEqual(before.composer);
  expect(await toolbar.boundingBox()).toEqual(before.toolbar);
  current.providerCalls[1] = { ...current.providerCalls[1], status: "completed", usage: { ...usage, estimatedCostUsd: "0.009007" } };
  state.status = "completed";
  await expect(chat.getByText("Kafka", { exact: true })).toBeVisible();
  await expect(chat.getByLabel("Provider request costs")).toContainText("2 provider requests · estimate: $0.019010");
  await expect(chat.getByRole("button", { name: "Copy response" })).toBeEnabled();
  expect(await stop.boundingBox()).toEqual(before.stop);
  expect(await send.boundingBox()).toEqual(before.send);
  expect(await composer.boundingBox()).toEqual(before.composer);
  expect(await toolbar.boundingBox()).toEqual(before.toolbar);
  expect(state.mutations.filter(call => call.path.includes("/chat"))).toHaveLength(0);
});

test("failed verification retries explicitly and Stop responds immediately without moving controls", async ({ page }) => {
  const state = await fixture(page);
  const current: ArticleChat = { ...exampleChat(), status: "failed", error: "Verification provider timeout", messages: [{ ...exampleChat().messages[0], status: "failed", content: "Rejected verification draft" }], providerCalls: [{ id: "generation", assistantId: "assistant", phase: "generating", status: "completed" }, { id: "verification", assistantId: "assistant", phase: "verifying", status: "failed" }] };
  state.status = "failed";
  state.versions.push(current);
  let releaseRetry!: () => void, releaseStop!: () => void;
  const retryGate = new Promise<void>(resolve => { releaseRetry = resolve; }), stopGate = new Promise<void>(resolve => { releaseStop = resolve; });
  let retries = 0, stops = 0;
  await page.route("**/api/ai/chats/*/retry", async route => {
    retries++;
    await retryGate;
    state.status = "verifying"; current.status = "verifying"; delete current.error;
    current.messages.push({ ...current.messages[0], id: "retry-assistant", status: "pending" });
    current.providerCalls.push({ id: "retry-verification", assistantId: "retry-assistant", phase: "verifying", status: "started" });
    await route.fulfill({ json: current });
  });
  await page.route("**/api/ai/chats/*/stop", async route => {
    stops++;
    await stopGate;
    state.status = "cancelled"; current.status = "cancelled"; current.messages[1].status = "interrupted";
    current.providerCalls[2].status = "cancelled";
    await route.fulfill({ json: current });
  });
  await page.goto("/reader");
  await page.getByRole("button", { name: "Summarize", exact: true }).click();
  const chat = page.getByRole("dialog", { name: "Article chat: Article A", exact: true });
  const retry = chat.getByRole("button", { name: "Retry", exact: true }), stop = chat.getByRole("button", { name: "Stop", exact: true }), composer = chat.getByLabel("Message DeepSeek");
  await expect(chat.getByRole("status")).toContainText("Verification failed");
  await expect(chat.getByRole("status")).toContainText("Verification provider timeout");
  await expect(chat.getByText("Rejected verification draft")).toHaveCount(0);
  const before = { retry: await retry.boundingBox(), stop: await stop.boundingBox(), composer: await composer.boundingBox() };
  await composer.fill("Retain my question");
  await expect(chat.getByRole("button", { name: "Send", exact: true })).toBeDisabled();
  await expect(chat.getByText(/Chat unlocks after a verified summary/)).toBeVisible();
  await composer.press("Control+Enter");
  expect(state.mutations.filter(call => call.path.endsWith("/messages"))).toHaveLength(0);
  expect(await composer.boundingBox()).toEqual(before.composer);
  await retry.dblclick();
  await expect(chat.getByRole("status")).toContainText("Retrying verification");
  await expect(retry).toBeDisabled();
  expect(retries).toBe(1);
  expect(await retry.boundingBox()).toEqual(before.retry);
  expect(await stop.boundingBox()).toEqual(before.stop);
  releaseRetry();
  await expect(chat.getByRole("status")).toContainText("Step 2 of 2");
  await expect(chat.getByLabel("Provider request costs")).toContainText("3 provider requests");
  await stop.dblclick();
  await expect(chat.getByRole("status")).toContainText("Stopping");
  await expect(stop).toBeDisabled();
  expect(stops).toBe(1);
  expect(await stop.boundingBox()).toEqual(before.stop);
  expect(await composer.boundingBox()).toEqual(before.composer);
  releaseStop();
  await expect(chat.getByRole("status")).toContainText("Stopped during verification");
  await expect(chat.getByRole("status")).toHaveAttribute("aria-busy", "false");
  await expect(chat.getByText("Rejected verification draft")).toHaveCount(0);
  await expect(composer).toHaveValue("Retain my question");
  expect(await retry.boundingBox()).toEqual(before.retry);
  expect(await stop.boundingBox()).toEqual(before.stop);
  expect(await composer.boundingBox()).toEqual(before.composer);
});
