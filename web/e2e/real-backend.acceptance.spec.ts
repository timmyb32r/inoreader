import { expect, test } from "@playwright/test";

test("real Rust API commits browser mutations and isolates account data", async ({
  page,
  context,
  baseURL,
}) => {
  const token = process.env.READER_ACCEPTANCE_TOKEN;
  if (!token || !baseURL)
    throw new Error("Hermetic acceptance fixture must provide session and URL");
  await context.addCookies([
    {
      name: "reader_session",
      value: token,
      url: baseURL,
      httpOnly: true,
      sameSite: "Strict",
    },
  ]);
  const privateResponse = await context.request.get(
    `/api/articles/${process.env.READER_PRIVATE_ARTICLE}?workspace_id=${process.env.READER_PRIVATE_WORKSPACE}`,
  );
  expect(privateResponse.status()).toBe(404);
  await page.goto("/reader");
  await expect(
    page.getByRole("heading", { name: /^Feed \(\d+\)$/ }),
  ).toBeVisible();
  await expect(page.locator(".article-row").first()).toContainText(
    "Persistent browser article",
  );
  const later = page
    .locator(".reader-toolbar")
    .getByRole("button", { name: "Read later", exact: true });
  await Promise.all([
    page.waitForResponse(
      (response) =>
        response.url().includes("/state") &&
        response.request().method() === "POST" &&
        response.status() === 200,
    ),
    later.click(),
  ]);
  await expect(
    page.getByRole("button", { name: "Remove from later", exact: true }),
  ).toBeVisible();
  await page.reload();
  await expect(
    page.getByRole("button", { name: "Remove from later", exact: true }),
  ).toBeVisible();
  const bootstrap = await context.request.get("/api/bootstrap");
  expect(bootstrap.headers()["x-request-id"]).toMatch(/^[0-9a-f-]{36}$/);
  expect((await bootstrap.json()).articlePage.articles[0].later).toBe(true);
});

test("translation traverses real HTTP, worker and PostgreSQL, retains failure and retries", async ({
  page,
  context,
  baseURL,
}) => {
  const token = process.env.READER_ACCEPTANCE_TOKEN!;
  await context.addCookies([
    {
      name: "reader_session",
      value: token,
      url: baseURL!,
      httpOnly: true,
      sameSite: "Strict",
    },
  ]);
  await page.goto("/reader");
  await page.getByRole("button", { name: "Close chat" }).click();
  const toggle = page.getByRole("button", {
    name: "Translate paragraphs",
    exact: true,
  });
  await expect(toggle).toBeEnabled();
  await toggle.click();
  const paragraph = page
    .locator(".article-content p")
    .filter({ hasText: "Exact source text" });
  await paragraph.click();
  const panel = page.getByRole("region", { name: "Paragraph translation" });
  await expect(panel).toHaveAttribute("aria-busy", "true");
  const before = await panel.boundingBox();
  await paragraph.click();
  await expect(panel.getByRole("alert")).toBeVisible();
  const after = await panel.boundingBox();
  expect(after).toEqual(before);
  await panel.getByRole("button", { name: "Повторить" }).click();
  await expect(panel).toHaveAttribute("aria-busy", "true");
  await expect(panel).toContainText("Точный исходный текст.");
  await page.reload();
  await page.getByRole("button", { name: "Close chat" }).click();
  await page
    .getByRole("button", { name: "Translate paragraphs", exact: true })
    .click();
  await page
    .locator(".article-content p")
    .filter({ hasText: "Exact source text" })
    .click();
  await expect(
    page.getByRole("region", { name: "Paragraph translation" }),
  ).toContainText("Точный исходный текст.");
  const bootstrap = await (await context.request.get("/api/bootstrap")).json();
  const ws = bootstrap.activeWorkspaceId,
    article = bootstrap.articlePage.articles[0].id;
  const jobs = await (
    await context.request.get(
      `/api/articles/${article}/translations?workspace_id=${ws}`,
    )
  ).json();
  expect(jobs).toHaveLength(2);
  expect(jobs.map((job: { status: string }) => job.status).sort()).toEqual([
    "completed",
    "failed",
  ]);
});

test("reader HTTP latency and startup request budgets", async ({
  page,
  context,
  baseURL,
}, info) => {
  const { readFileSync } = await import("node:fs");
  const budgets = JSON.parse(
    readFileSync(
      new URL("../../docs/architecture/latency-budgets.json", import.meta.url),
      "utf8",
    ),
  );
  await context.addCookies([
    {
      name: "reader_session",
      value: process.env.READER_ACCEPTANCE_TOKEN!,
      url: baseURL!,
      httpOnly: true,
      sameSite: "Strict",
    },
  ]);
  let articleRequests = 0;
  page.on("request", (request) => {
    if (/\/api\/articles(?:\?|\/[^/]+\?)/.test(request.url()))
      articleRequests++;
  });
  await page.goto("/reader");
  await expect(page.locator(".article-content")).toContainText(
    "Exact source text",
  );
  expect(articleRequests).toBeLessThanOrEqual(
    budgets.maxInitialArticleRequests,
  );
  const bootstrap = await (await context.request.get("/api/bootstrap")).json();
  expect(bootstrap.articlePage.articles.length).toBeLessThanOrEqual(
    budgets.maxPageArticles,
  );
  const workspace = bootstrap.activeWorkspaceId,
    article = bootstrap.articlePage.articles[0].id;
  const results: Record<string, { p95Ms: number; samplesMs: number[] }> = {};
  for (const route of [
    "/api/bootstrap",
    `/api/articles?workspace_id=${workspace}&view=feed`,
    `/api/subscriptions?workspace_id=${workspace}`,
    `/api/articles/${article}?workspace_id=${workspace}`,
  ]) {
    const samples: number[] = [];
    for (let i = 0; i < 23; i++) {
      const started = performance.now();
      const response = await context.request.get(route);
      expect(response.ok()).toBe(true);
      await response.body();
      if (i >= 3) samples.push(performance.now() - started);
    }
    samples.sort((a, b) => a - b);
    const key = route.startsWith(`/api/articles/${article}`)
      ? "/api/articles/{id}"
      : route.split("?")[0];
    results[key] = {
      p95Ms: samples[Math.ceil(samples.length * 0.95) - 1],
      samplesMs: samples,
    };
    expect(results[key].p95Ms, key).toBeLessThan(budgets.apiP95Ms);
  }
  await info.attach("reader-latency.json", {
    body: JSON.stringify(results, null, 2),
    contentType: "application/json",
  });
});
