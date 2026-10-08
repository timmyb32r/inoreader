import { expect, test } from "@playwright/test";
async function account(page: import("@playwright/test").Page, name: string) {
  await page.route("**/api/bootstrap*", async (route) => {
    const response = await route.fetch();
    const data = await response.json();
    data.account.displayName = name;
    await route.fulfill({ json: data });
  });
}
for (const width of [1440, 390])
  test(`DeepSeek statistics filters, drilldown and stable pending controls at ${width}px`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 900 });
    await account(page, "timmyb32r");
    let calls = 0,
      hold = false;
    await page.route("**/api/ai/statistics?*", async (route) => {
      calls++;
      while (hold) await new Promise((resolve) => setTimeout(resolve, 20));
      const q = new URL(route.request().url()).searchParams;
      const from = q.get("from")!,
        until = q.get("until")!,
        bucket = q.get("bucket")!;
      const period =
        bucket === "year"
          ? from.slice(0, 4) + "-01-01"
          : bucket === "month"
            ? from.slice(0, 7) + "-01"
            : from;
      await route.fulfill({
        json: {
          from,
          until,
          bucket,
          timezone: "Europe/Moscow",
          rows: [
            {
              period,
              mode: "summary",
              requests: 8,
              unconfirmed: 1,
              spentUsd: "0.123456789",
              reservedUsd: "0.5",
            },
            {
              period,
              mode: "terms",
              requests: 4,
              unconfirmed: 0,
              spentUsd: "0.000000001",
              reservedUsd: "0",
            },
            {
              period,
              mode: "ranking",
              requests: 2,
              unconfirmed: 0,
              spentUsd: "0",
              reservedUsd: "0",
            },
          ],
        },
      });
    });
    await page.goto(
      "/ai-statistics?from=2026-10-01&until=2026-10-08&bucket=day",
    );
    await expect(
      page.getByRole("heading", { name: "DeepSeek / статистика" }),
    ).toBeVisible();
    await expect(page.locator(".ds-kpis")).toContainText("14");
    await expect(page.locator(".ds-kpis")).toContainText("$0.123456790");
    await expect(
      page.getByRole("button", { name: "2026-10-02: 0 запросов" }),
    ).toBeVisible();
    const start = page.getByLabel("Начало периода"),
      end = page.getByLabel("Конец периода"),
      show = page.getByRole("button", { name: "Показать", exact: true });
    await start.fill("2026-09-01");
    await end.fill("2026-09-30");
    const geometry = await show.boundingBox(),
      startBox = await start.boundingBox(),
      endBox = await end.boundingBox();
    const chartHeight = await page
      .locator(".ds-chart-scroll")
      .evaluate((element) => element.clientHeight);
    const initial = calls;
    hold = true;
    await show.click();
    await expect(
      page.getByRole("button", { name: "Показать", exact: true }),
    ).toHaveAttribute("aria-busy", "true");
    await expect(start).toBeDisabled();
    await expect(page.locator(".ds-tooltip")).toHaveCount(0);
    await show.evaluate((element) => {
      if (element instanceof HTMLButtonElement) {
        element.click();
        element.click();
      }
    });
    await expect.poll(() => calls).toBe(initial + 1);
    expect(await show.boundingBox()).toEqual(geometry);
    expect(await start.boundingBox()).toEqual(startBox);
    expect(await end.boundingBox()).toEqual(endBox);
    hold = false;
    await expect(show).toBeEnabled();
    expect(await show.boundingBox()).toEqual(geometry);
    expect(await start.boundingBox()).toEqual(startBox);
    expect(await end.boundingBox()).toEqual(endBox);
    await page.getByRole("button", { name: "По годам", exact: true }).click();
    await expect(
      page.getByRole("button", { name: "По годам", exact: true }),
    ).toHaveAttribute("aria-pressed", "true");
    await expect(show).toBeEnabled();
    expect(
      await page
        .locator(".ds-chart-scroll")
        .evaluate((element) => element.clientHeight),
    ).toBe(chartHeight);
    const year = page.locator(".ds-column").first();
    await year.click();
    await expect(
      page.getByRole("button", { name: "По месяцам", exact: true }),
    ).toHaveAttribute("aria-pressed", "true");
    await expect(show).toBeEnabled();
    await page.locator(".ds-column").first().click();
    await expect(
      page.getByRole("button", { name: "По дням", exact: true }),
    ).toHaveAttribute("aria-pressed", "true");
    await expect(show).toBeEnabled();
    const beforeInvalid = calls;
    await start.fill("2026-12-31");
    await end.fill("2026-01-01");
    await show.click();
    await expect(page.locator(".ds-status")).toContainText(
      "корректный диапазон",
    );
    expect(calls).toBe(beforeInvalid);
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth),
    ).toBeLessThanOrEqual(width);
    await page.screenshot({
      path: `/private/tmp/deepseek-statistics-${width}.png`,
      fullPage: true,
    });
  });
test("foreign user has neither navigation entry nor statistics request", async ({
  page,
}) => {
  await account(page, "other-admin");
  let requests = 0;
  await page.route("**/api/ai/statistics?*", (route) => {
    requests++;
    return route.fulfill({
      status: 403,
      json: { code: "forbidden", message: "Forbidden" },
    });
  });
  await page.goto("/ai-statistics");
  await expect(
    page.getByRole("heading", { name: "Нет доступа" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "DeepSeek stats" }),
  ).toHaveCount(0);
  expect(requests).toBe(0);
});
test("statistics failures retain chart footprint and expose retry", async ({
  page,
}) => {
  await account(page, "timmyb32r");
  let fail = true;
  await page.route("**/api/ai/statistics?*", (route) =>
    fail
      ? route.fulfill({
          status: 503,
          json: { code: "unavailable", message: "Temporary failure" },
        })
      : route.fulfill({
          json: {
            from: "2026-10-01",
            until: "2026-10-01",
            bucket: "day",
            timezone: "Europe/Moscow",
            rows: [],
          },
        }),
  );
  await page.goto("/ai-statistics?from=2026-10-01&until=2026-10-01&bucket=day");
  const show = page.getByRole("button", { name: "Показать", exact: true });
  await expect(page.locator(".ds-status")).toContainText("Temporary failure");
  const button = await show.boundingBox(),
    chart = await page.locator(".ds-chart").boundingBox();
  fail = false;
  await show.click();
  await expect(page.getByText("В этом периоде запросов нет.")).toBeVisible();
  expect(await show.boundingBox()).toEqual(button);
  expect(await page.locator(".ds-chart").boundingBox()).toEqual(chart);
});
