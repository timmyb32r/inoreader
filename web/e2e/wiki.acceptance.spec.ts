import { expect, test } from "@playwright/test";
test("private wiki persists exact Markdown, rejects stale writes, and has stable save controls", async ({
  page,
  context,
  baseURL,
}) => {
  const token = process.env.READER_ACCEPTANCE_TOKEN;
  if (!token || !baseURL) throw new Error("Missing hermetic fixture");
  await context.addCookies([
    {
      name: "reader_session",
      value: token,
      url: baseURL,
      httpOnly: true,
      sameSite: "Strict",
    },
  ]);
  const headers = { Origin: baseURL, "Content-Type": "application/json" };
  const id = crypto.randomUUID();
  const created = await context.request.post("/api/wiki/namespaces", {
    headers,
    data: { id, name: "Acceptance wiki" },
  });
  expect(created.status()).toBe(200);
  expect(created.headers()["cache-control"]).toBe("no-store");
  const missing = await context.request.get(
    `/api/wiki/${crypto.randomUUID()}/pages`,
  );
  expect(missing.status()).toBe(404);
  const csrf = await context.request.post("/api/wiki/namespaces", {
    headers: { Origin: "https://evil.example" },
    data: { id: crypto.randomUUID(), name: "Forbidden" },
  });
  expect(csrf.status()).toBe(403);
  await page.goto(`/wiki/${id}`);
  await expect(
    page.getByRole("heading", { name: "Acceptance wiki" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "New page", exact: true }).click();
  await page.getByRole("textbox", { name: "Page name" }).fill(" Exact 页面 ");
  const markdown =
    "# Heading\n\nOriginal 中文 Русский\n\n[[Missing page]]\n\n<script>alert('never')</script>";
  await page.getByRole("textbox", { name: "Markdown" }).fill(markdown);
  await expect(
    page.getByText("Private draft saved", { exact: true }),
  ).toBeVisible();
  await page.reload();
  await expect(page.getByRole("textbox", { name: "Markdown" })).toHaveValue(
    markdown,
  );
  let release!: () => void;
  const gate = new Promise<void>((r) => {
    release = r;
  });
  let writes = 0;
  await page.route(`**/api/wiki/${id}/pages`, async (route) => {
    if (route.request().method() === "POST") {
      writes++;
      await gate;
    }
    await route.continue();
  });
  const save = page.getByRole("button", { name: "Save", exact: true }),
    cancel = page.getByRole("button", { name: "Cancel", exact: true });
  const before = {
    save: await save.boundingBox(),
    cancel: await cancel.boundingBox(),
  };
  await save.click();
  await expect(save).toHaveAttribute("aria-busy", "true");
  expect(await save.boundingBox()).toEqual(before.save);
  expect(await cancel.boundingBox()).toEqual(before.cancel);
  await save.dispatchEvent("click");
  expect(writes).toBe(1);
  release();
  await expect(
    page.getByRole("heading", { name: "Exact 页面", exact: true }),
  ).toBeVisible();
  expect(writes).toBe(1);
  await page.unroute(`**/api/wiki/${id}/pages`);
  await page.getByRole("button", { name: "Missing page", exact: true }).click();
  await expect(
    page.getByRole("dialog", { name: "Missing wiki page" }),
  ).toBeVisible();
  const list = await (
    await context.request.get(`/api/wiki/${id}/pages`)
  ).json();
  expect(list.items).toHaveLength(1);
  await page.getByRole("button", { name: "Create page", exact: true }).click();
  await expect(page.getByRole("textbox", { name: "Page name" })).toHaveValue(
    "Missing page",
  );
  const existing = await (
    await context.request.get(`/api/wiki/${id}/pages/${list.items[0].id}`)
  ).json();
  const command = {
    operation: crypto.randomUUID(),
    page: existing.id,
    expected_revision: existing.revision,
    change: { action: "rename", name: "Renamed" },
  };
  const changed = await context.request.post(`/api/wiki/${id}/pages`, {
    headers,
    data: command,
  });
  expect(changed.status()).toBe(200);
  expect(
    (
      await context.request.post(`/api/wiki/${id}/pages`, {
        headers,
        data: command,
      })
    ).status(),
  ).toBe(200);
  expect(
    (
      await context.request.post(`/api/wiki/${id}/pages`, {
        headers,
        data: { ...command, operation: crypto.randomUUID() },
      })
    ).status(),
  ).toBe(409);
  const alias = await context.request.get(`/api/wiki/${id}/resolve`, {
    params: { name: " Exact 页面 " },
  });
  expect((await alias.json()).name).toBe("Renamed");
  await page.goto(`/wiki/${id}/page/${existing.id}`);
  await expect(
    page.getByRole("heading", { name: "Renamed", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Missing page", exact: true }),
  ).toHaveClass(/wiki-link--missing/);
  await page.screenshot({ path: test.info().outputPath("wiki-desktop.png") });
  await page.getByRole("button", { name: "Use dark theme" }).click();
  await page.screenshot({ path: test.info().outputPath("wiki-dark.png") });
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(
    page.getByRole("heading", { name: "Renamed", exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(() => document.documentElement.scrollWidth),
  ).toBeLessThanOrEqual(390);
  const topbar = (await page.locator(".topbar").boundingBox())!;
  await expect
    .poll(async () =>
      Math.round((await page.locator(".wiki-shell").boundingBox())!.y),
    )
    .toBe(Math.round(topbar.y + topbar.height));
  await expect
    .poll(async () => {
      const box = (await page.locator(".sidebar").boundingBox())!;
      return box.x + box.width;
    })
    .toBeLessThanOrEqual(0);
  await page.screenshot({ path: test.info().outputPath("wiki-mobile.png") });
  const history = await (
    await context.request.get(`/api/wiki/${id}/pages/${existing.id}/history`)
  ).json();
  expect(history.items).toHaveLength(2);
  // Returning from a linked Wiki page must preserve the original popup ancestry.
  const bootstrap = await (await context.request.get("/api/bootstrap")).json();
  const subscription = bootstrap.subscriptions[0].id;
  expect(
    (
      await context.request.put(`/api/subscriptions/${subscription}/wiki`, {
        headers,
        data: { target: { namespace: id, page: existing.id } },
      })
    ).status(),
  ).toBe(204);
  await page.setViewportSize({ width: 1280, height: 720 });
  await page.goto(`/reader?view=subscription&subscription=${subscription}`);
  await page
    .getByRole("link", { name: "Open subscription Browser AI" })
    .click();
  await expect(page).toHaveURL(new RegExp(`/subscriptions/${subscription}$`));
  await page
    .locator(".wiki-binding")
    .getByRole("link", { name: "Open", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Renamed", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Namespaces", exact: false }).click();
  await page
    .getByRole("button", { name: "Back to Reader", exact: false })
    .click();
  await expect(page).toHaveURL(new RegExp(`/subscriptions/${subscription}$`));
  await page.getByRole("button", { name: "Close subscriptions" }).click();
  await expect(page).toHaveURL(
    (url) =>
      url.pathname === "/reader" &&
      url.searchParams.get("view") === "subscription" &&
      url.searchParams.get("subscription") === subscription &&
      url.searchParams.has("article") &&
      url.searchParams.get("workspace") === bootstrap.activeWorkspaceId,
  );
  await expect(
    page.getByRole("dialog", { name: "Subscription details" }),
  ).toHaveCount(0);
});

test("unified search uses real API authorization and namespace-scoped wiki entry", async ({
  page,
  context,
  baseURL,
}) => {
  const token = process.env.READER_ACCEPTANCE_TOKEN;
  if (!token || !baseURL) throw new Error("Missing hermetic fixture");
  await context.addCookies([
    {
      name: "reader_session",
      value: token,
      url: baseURL,
      httpOnly: true,
      sameSite: "Strict",
    },
  ]);
  const headers = { Origin: baseURL, "Content-Type": "application/json" };
  const namespace = crypto.randomUUID(),
    id = crypto.randomUUID();
  expect(
    (
      await context.request.post("/api/wiki/namespaces", {
        headers,
        data: { id: namespace, name: "Search acceptance" },
      })
    ).status(),
  ).toBe(200);
  expect(
    (
      await context.request.post(`/api/wiki/${namespace}/pages`, {
        headers,
        data: {
          operation: crypto.randomUUID(),
          page: id,
          expected_revision: null,
          change: {
            action: "save",
            name: "SearchToken 页面",
            markdown: "Only this published text is searchable",
          },
        },
      })
    ).status(),
  ).toBe(200);
  const response = await context.request.get(
    "/api/search?q=SearchToken&kind=all",
  );
  expect(response.status()).toBe(200);
  expect(response.headers()["cache-control"]).toBe("no-store");
  const results = await response.json();
  expect(
    results.items.some(
      (hit: { target: { page?: string } }) => hit.target.page === id,
    ),
  ).toBe(true);
  expect(
    (
      await context.request.get(
        `/api/search?q=SearchToken&kind=wiki&namespace=${crypto.randomUUID()}`,
      )
    ).status(),
  ).toBe(404);
  expect(
    (
      await context.request.get(
        `/api/search?q=SearchToken&kind=news&namespace=${namespace}`,
      )
    ).status(),
  ).toBe(422);
  let releaseListing!: () => void;
  const listingGate = new Promise<void>((resolve) => {
    releaseListing = resolve;
  });
  await page.route(`**/api/wiki/${namespace}/pages?*`, async (route) => {
    await listingGate;
    await route.continue();
  });
  await page.goto(`/wiki/${namespace}`);
  await page
    .getByRole("searchbox", { name: "Search this wiki" })
    .fill("SearchToken");
  releaseListing();
  await expect(
    page
      .locator(".wiki-page-list")
      .getByRole("button", { name: /SearchToken/ }),
  ).toBeVisible();
  await expect(
    page.getByRole("searchbox", { name: "Search this wiki" }),
  ).toHaveValue("SearchToken");
  await page
    .getByRole("button", { name: "Search", exact: true })
    .last()
    .click();
  await expect.poll(() => new URL(page.url()).pathname).toBe("/search");
  expect(new URL(page.url()).searchParams.get("namespace")).toBe(namespace);
  expect(new URL(page.url()).searchParams.get("q")).toBe("SearchToken");
  await page.getByRole("button", { name: /SearchToken 页面/ }).click();
  await expect(page.getByLabel("Search preview")).toContainText(
    "Only this published text is searchable",
  );
});
