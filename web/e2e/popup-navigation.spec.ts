import { expect, test } from "@playwright/test";

for (const readerUrl of [
  "/reader",
  "/reader?view=subscription&subscription=sub",
]) {
  test(`article source opens its subscription and returns without shifting the reader: ${readerUrl}`, async ({
    page,
  }) => {
    await page.goto(readerUrl);
    const reader = page.getByRole("article", { name: "Article reader" });
    const source = reader.getByRole("link", {
      name: "Open subscription This Week in Rust",
    });
    await expect(source).toHaveAttribute("href", "/subscriptions/sub");
    const title = await reader.locator("h1").textContent();
    const url = page.url();
    const bounds = await source.boundingBox();
    const toolbarBounds = await reader.locator(".reader-toolbar").boundingBox();
    const normalBackground = await source.evaluate(
      (element) => getComputedStyle(element).backgroundColor,
    );
    await source.hover();
    await expect(source).not.toHaveCSS("background-color", normalBackground);
    expect(await source.boundingBox()).toEqual(bounds);
    const hoverBackground = await source.evaluate(
      (element) => getComputedStyle(element).backgroundColor,
    );
    await page.mouse.down();
    await expect(source).not.toHaveCSS("background-color", hoverBackground);
    expect(await source.boundingBox()).toEqual(bounds);
    await page.mouse.up();
    await expect(
      page.getByRole("dialog", { name: "Subscription details", exact: true }),
    ).toBeVisible();
    await expect(page).toHaveURL(/\/subscriptions\/sub$/);
    expect(await reader.locator(".reader-toolbar").boundingBox()).toEqual(
      toolbarBounds,
    );
    await page.getByRole("button", { name: "Close subscriptions" }).click();
    await expect(page).toHaveURL(url);
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(reader.locator("h1")).toHaveText(title!);
    await expect(source).toBeFocused();
    expect(await source.boundingBox()).toEqual(bounds);
    await source.press("Enter");
    await expect(page.getByRole("dialog")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page).toHaveURL(url);
    await source.locator(".source__mark").click();
    await expect(page).toHaveURL(/\/subscriptions\/sub$/);
  });
}

test("direct subscription popup returns to the same reader with stable geometry", async ({
  page,
}) => {
  await page.goto("/reader");
  await page
    .getByRole("navigation", { name: "Subscriptions", exact: true })
    .getByRole("button", { name: /This Week in Rust/ })
    .click();
  const heading = page.getByRole("link", {
    name: "Open settings for This Week in Rust",
  });
  await expect(page).toHaveURL(/subscription=sub/);
  const url = page.url();
  const bounds = await heading.boundingBox();
  await heading.click();
  await expect(
    page.getByRole("dialog", { name: "Subscription details", exact: true }),
  ).toBeVisible();
  expect(await heading.boundingBox()).toEqual(bounds);
  await page.getByRole("button", { name: "Close subscriptions" }).click();
  await expect(page).toHaveURL(url);
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(await heading.boundingBox()).toEqual(bounds);
  await expect(heading).toBeFocused();
  await heading.click();
  await page.keyboard.press("Escape");
  await expect(page).toHaveURL(url);
});

test("catalog returns to Home and nested dialogs close only one level", async ({
  page,
}) => {
  await page.goto("/");
  const home = page.getByRole("button", { name: "Home", exact: true });
  const bounds = await home.boundingBox();
  await page.getByRole("button", { name: /^Subscriptions \d/ }).click();
  await page
    .getByRole("searchbox", { name: "Search subscriptions" })
    .fill("Rust");
  await page
    .getByRole("link", { name: "This Week in Rust", exact: true })
    .click();
  await page.getByRole("button", { name: "Edit name" }).click();
  await page.keyboard.press("Escape");
  await expect(
    page.getByRole("dialog", { name: "Edit custom name", exact: true }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("dialog", { name: "Subscription details", exact: true }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(
    page.getByRole("searchbox", { name: "Search subscriptions" }),
  ).toHaveValue("Rust");
  await page.keyboard.press("Escape");
  await expect(page).toHaveURL("/");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(await home.boundingBox()).toEqual(bounds);
});
