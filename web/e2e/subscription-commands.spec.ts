import { expect, test } from "@playwright/test";

test("resume immediately locks its control, keeps targets fixed and reports failures", async ({
  page,
}) => {
  await page.route("**/api/bootstrap*", async (route) => {
    const data = await (await route.fetch()).json();
    data.subscriptions[0].status = "paused";
    await route.fulfill({ json: data });
  });
  let release!: () => void;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  let calls = 0;
  await page.route("**/api/subscriptions/sub/resume", async (route) => {
    calls++;
    await gate;
    await route.fulfill({
      status: 503,
      json: { error: "Temporarily unavailable" },
    });
  });
  await page.goto("/reader");
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await page.getByLabel("Choose subscription").selectOption("sub");
  const resume = page.getByRole("button", { name: "Resume This Week in Rust" });
  await resume.scrollIntoViewIfNeeded();
  const unsubscribe = page.getByRole("button", {
    name: "Unsubscribe",
    exact: true,
  });
  const before = await resume.boundingBox(),
    next = await unsubscribe.boundingBox();
  await resume.click();
  await expect(resume).toHaveAttribute("aria-busy", "true");
  await expect(resume).toBeDisabled();
  await resume.dispatchEvent("click");
  await expect.poll(() => calls).toBe(1);
  expect(await resume.boundingBox()).toEqual(before);
  expect(await unsubscribe.boundingBox()).toEqual(next);
  release();
  await expect(resume).toBeEnabled();
  await expect(
    page.getByRole("dialog").getByRole("status").last(),
  ).not.toBeEmpty();
  expect(await resume.boundingBox()).toEqual(before);
  expect(await unsubscribe.boundingBox()).toEqual(next);
});

test("sidebar refresh locks immediately and its failure does not move Pause", async ({
  page,
}) => {
  let release!: () => void;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  let calls = 0;
  await page.route("**/api/subscriptions/sub/refresh", async (route) => {
    calls++;
    await gate;
    await route.fulfill({
      status: 503,
      json: { error: "Temporarily unavailable" },
    });
  });
  await page.goto("/reader");
  await page
    .getByRole("button", { name: "More options", exact: true })
    .first()
    .click();
  const menu = page.locator(".source-menu"),
    refresh = menu.getByRole("button", { name: "Refresh", exact: true });
  const pause = menu.getByRole("button", { name: "Pause", exact: true });
  await refresh.scrollIntoViewIfNeeded();
  const before = await refresh.boundingBox(),
    next = await pause.boundingBox();
  await refresh.click();
  await expect(refresh).toHaveAttribute("aria-busy", "true");
  await refresh.dispatchEvent("click");
  await expect.poll(() => calls).toBe(1);
  expect(await refresh.boundingBox()).toEqual(before);
  expect(await pause.boundingBox()).toEqual(next);
  release();
  await expect(refresh).toBeEnabled();
  await expect(menu.getByRole("status")).not.toBeEmpty();
  expect(await refresh.boundingBox()).toEqual(before);
  expect(await pause.boundingBox()).toEqual(next);
});

test("late resume completion preserves a different dialog opened after Settings closes", async ({
  page,
}) => {
  await page.route("**/api/bootstrap*", async (route) => {
    const data = await (await route.fetch()).json();
    data.subscriptions[0].status = "paused";
    await route.fulfill({ json: data });
  });
  let release!: () => void;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/api/subscriptions/sub/resume", async (route) => {
    await gate;
    await route.fulfill({ status: 204 });
  });
  await page.goto("/reader");
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await page.getByLabel("Choose subscription").selectOption("sub");
  const resume = page.getByRole("button", { name: "Resume This Week in Rust" });
  await resume.click();
  await expect(resume).toHaveAttribute("aria-busy", "true");
  await page.keyboard.press("Escape");
  await page
    .getByRole("button", { name: "Add subscription", exact: true })
    .click();
  const dialog = page.getByRole("dialog", {
    name: "Add a subscription",
    exact: true,
  });
  await expect(dialog).toBeVisible();
  const loaded = page.waitForResponse(
    (response) =>
      response.url().endsWith("/api/subscriptions/sub") &&
      response.request().method() === "GET",
  );
  release();
  await loaded;
  await expect(dialog).toBeVisible();
  await expect(page.getByLabel("Feed or website URL")).toBeVisible();
});
