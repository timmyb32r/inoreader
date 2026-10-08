import { expect, test } from "@playwright/test";

for (const trigger of ["card", "bar", "calendar"] as const) {
  test(`reading history ${trigger} keeps controls stable while pending, then opens filtered Feed and persists on reload`, async ({
    page,
  }) => {
    let release!: () => void;
    let gated = true,
      requests = 0;
    let period: URL | undefined;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const day = "2026-09-29";
    await page.clock.setFixedTime(new Date("2026-09-29T12:00:00Z"));
    await page.route("**/api/workspaces/*/reading-activity?*", (r) =>
      r.fulfill({ json: { days: [{ day, count: 2, arrived: 7 }] } }),
    );
    await page.route("**/api/articles?*", async (route) => {
      const url = new URL(route.request().url());
      const response = await route.fetch();
      const body = await response.json();
      if (url.searchParams.has("read_from")) {
        requests++;
        period = url;
        if (gated) await gate;
        body.articles.forEach((a: { read: boolean; markedReadAt: string }) => {
          a.read = true;
          a.markedReadAt = "2026-09-29T10:18:00Z";
        });
      }
      if (!url.searchParams.has("read_from")) {
        body.articles = body.articles.slice(0, 1);
        body.total = 1;
      }
      await route.fulfill({ json: body });
    });
    await page.route("**/api/bootstrap?*", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      if (new URL(route.request().url()).searchParams.has("read_from")) {
        body.articlePage.articles.forEach(
          (a: { read: boolean; markedReadAt: string }) => {
            a.read = true;
            a.markedReadAt = "2026-09-29T10:18:00Z";
          },
        );
      }
      await route.fulfill({ json: body });
    });
    await page.goto("/");
    const card = page.getByRole("button", {
      name: /2 articles marked read today/,
    });
    const target =
      trigger === "card"
        ? card
        : trigger === "bar"
          ? page.getByRole("button", {
              name: "Show articles marked read on Sep 29, 2026",
            })
          : page.getByRole("gridcell", { name: "Sep 29, 2026: 2 articles" });
    await expect(target).toBeEnabled();
    await target.scrollIntoViewIfNeeded();
    const before = await target.boundingBox();
    const feed = page.getByRole("button", {
      name: "Начать чтение",
      exact: true,
    });
    const feedBefore = await feed.boundingBox();
    await target.click();
    await expect(target).toBeDisabled();
    await expect(card).toHaveAttribute("aria-busy", "true");
    expect(await target.boundingBox()).toEqual(before);
    expect(await feed.boundingBox()).toEqual(feedBefore);
    await expect.poll(() => requests).toBe(1);
    gated = false;
    release();
    await expect(page).toHaveURL(/\/reader\?read_from=.*read_until=/);
    await expect(page.getByLabel("Marked read on")).toHaveValue(day);
    await expect(
      page.getByText("Recently marked read", { exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Mark all read", exact: true }),
    ).toBeDisabled();
    expect(period?.searchParams.get("read_from")).toBeTruthy();
    await page.reload();
    await expect(page.getByLabel("Marked read on")).toHaveValue(day);
    await page.screenshot({ path: test.info().outputPath("read-history.png") });
    const stamp = await page.locator(".article-read-at").first().boundingBox();
    expect(stamp!.width).toBeGreaterThan(150);
    expect(stamp!.height).toBeLessThan(35);
    const dateBeforeClear = await page
      .getByLabel("Marked read on")
      .boundingBox();
    const listBeforeClear = await page.locator(".article-scroll").boundingBox();
    await page
      .getByRole("button", { name: "Clear reading date filter" })
      .click();
    await expect(page).not.toHaveURL(/read_from/);
    await expect(page.getByLabel("Marked read on")).toHaveValue("");
    expect(await page.getByLabel("Marked read on").boundingBox()).toEqual(
      dateBeforeClear,
    );
    expect(await page.locator(".article-scroll").boundingBox()).toEqual(
      listBeforeClear,
    );
    await expect(
      page.getByText("All unread articles", { exact: true }),
    ).toBeVisible();
  });
}

test("local calendar days preserve DST boundaries", async ({ browser }) => {
  const context = await browser.newContext({ timezoneId: "America/New_York" });
  const page = await context.newPage();
  await page.clock.setFixedTime(new Date("2026-03-08T12:00:00Z"));
  await page.route("**/api/workspaces/*/reading-activity?*", (r) =>
    r.fulfill({
      json: { days: [{ day: "2026-03-08", count: 1, arrived: 1 }] },
    }),
  );
  let from = "",
    until = "";
  await page.route("**/api/articles?*", async (route) => {
    const url = new URL(route.request().url());
    from = url.searchParams.get("read_from") ?? "";
    until = url.searchParams.get("read_until") ?? "";
    await route.continue();
  });
  await page.goto("/");
  await page
    .getByRole("button", { name: /1 articles marked read today/ })
    .click();
  await expect(page.getByLabel("Marked read on")).toBeVisible();
  expect(from).toBe("2026-03-08T05:00:00.000Z");
  expect(until).toBe("2026-03-09T04:00:00.000Z");
  await context.close();
});
