import { expect, test } from "@playwright/test";

test("Home arrivals opens today's proportional treemap and arbitrary date ranges keep controls stable", async ({
  page,
}) => {
  const today = new Date().toLocaleDateString("en-CA");
  await page.route("**/reading-activity?*", (route) =>
    route.fulfill({ json: { days: [{ day: today, count: 2, arrived: 10 }] } }),
  );
  let requests = 0,
    hold = false;
  await page.route("**/source-activity?*", async (route) => {
    requests++;
    while (hold) await new Promise((resolve) => setTimeout(resolve, 20));
    const query = new URL(route.request().url()).searchParams;
    await route.fulfill({
      json: {
        days: [
          {
            day: query.get("from"),
            total: 10,
            sources: [
              {
                subscriptionId: "sub",
                name: "This Week in Rust",
                present: true,
                count: 8,
              },
              {
                subscriptionId: "other",
                name: "Small source",
                present: true,
                count: 2,
              },
            ],
          },
        ],
      },
    });
  });
  await page.goto("/");
  await page.getByRole("button", { name: /articles arrived today/ }).click();
  await expect(page).toHaveURL(new RegExp(`/source-activity\\?day=${today}`));
  const start = page.getByLabel("Analysis start date"),
    end = page.getByLabel("Analysis end date");
  await expect(start).toHaveValue(today);
  await expect(end).toHaveValue(today);
  const large = page.getByRole("button", {
    name: "This Week in Rust: 8 articles · 80.0%",
  });
  const small = page.getByRole("button", {
    name: "Small source: 2 articles · 20.0%",
  });
  await expect(large).toBeVisible();
  const bigBox = (await large.boundingBox())!,
    smallBox = (await small.boundingBox())!;
  expect(
    (bigBox.width * bigBox.height) / (smallBox.width * smallBox.height),
  ).toBeCloseTo(4, 1);
  await small.hover();
  await expect(page.locator(".source-treemap-detail")).toContainText(
    "Small source: 2 articles",
  );
  const controls = await page
    .locator(".source-activity-controls")
    .boundingBox();
  hold = true;
  await start.fill("2020-01-01");
  const apply = page.getByRole("button", { name: "Apply", exact: true });
  await apply.click();
  await expect(
    page.getByRole("button", { name: "Loading…", exact: true }),
  ).toBeDisabled();
  expect(await page.locator(".source-activity-controls").boundingBox()).toEqual(
    controls,
  );
  await expect.poll(() => requests).toBe(2);
  hold = false;
  await expect(apply).toBeEnabled();
  expect(await page.locator(".source-activity-controls").boundingBox()).toEqual(
    controls,
  );
  await expect(page.locator(".source-activity-status")).toContainText(
    "2020-01-01",
  );
  await end.fill("2019-01-01");
  await expect(apply).toBeDisabled();
  expect(requests).toBe(2);
});
