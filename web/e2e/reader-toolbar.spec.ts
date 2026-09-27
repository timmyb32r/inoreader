import { expect, test } from "@playwright/test";
import { wireFixture } from "./wire-fixtures.mjs";

test("toolbar has consistent neutral controls and stable toggle/press styles in both themes", async ({
  page,
}) => {
  await page.route("**/api/ai/profile", (route) =>
    route.fulfill({
      json: wireFixture({ configured: true, enabled: true }),
    }),
  );
  await page.goto("/reader");
  const toolbar = page.locator(".reader-toolbar");
  const summarize = toolbar.getByRole("button", { name: "Summarize" });
  const terms = toolbar.getByRole("button", { name: "Terms" });
  const translate = toolbar.getByRole("button", {
    name: "Translate paragraphs",
  });
  await expect(translate).toBeEnabled();
  const bounds = () =>
    toolbar
      .locator("button:visible, a:visible")
      .evaluateAll((elements) =>
        elements.map((el) => el.getBoundingClientRect().toJSON()),
      );
  const before = await bounds();
  for (const theme of ["light", "dark"]) {
    if (theme === "dark")
      await page.getByRole("button", { name: "Use dark theme" }).click();
    await page.mouse.move(0, 0);
    const styles = await toolbar
      .locator("button:visible, a:visible")
      .evaluateAll((elements) =>
        elements.map((el) => {
          const style = getComputedStyle(el);
          return {
            height: style.height,
            font: style.font,
            borderRadius: style.borderRadius,
            borderWidth: style.borderWidth,
          };
        }),
      );
    expect(styles.every((style) => style.height === "36px")).toBe(true);
    expect(new Set(styles.map((style) => JSON.stringify(style))).size).toBe(1);
    const idle = await summarize.evaluate(
      (el) => getComputedStyle(el).backgroundColor,
    );
    await expect(terms).toHaveCSS("background-color", idle);
    await expect(translate).toHaveCSS("background-color", idle);
    await expect(toolbar.getByRole("link")).toHaveCSS("background-color", idle);
    await expect(summarize).not.toHaveAttribute("aria-pressed");
    await summarize.hover();
    await expect(summarize).not.toHaveCSS("background-color", idle);
    const hover = await summarize.evaluate(
      (el) => getComputedStyle(el).backgroundColor,
    );
    await page.mouse.down();
    await expect(summarize).not.toHaveCSS("background-color", hover);
    expect(await bounds()).toEqual(before);
    await page.mouse.move(0, 0);
    await page.mouse.up();
    await translate.click();
    await page.mouse.move(0, 0);
    await expect(translate).toHaveAttribute("aria-pressed", "true");
    await expect(translate).not.toHaveCSS("background-color", idle);
    expect(await bounds()).toEqual(before);
    await translate.press("Space");
    await expect(translate).toHaveAttribute("aria-pressed", "false");
    await expect(translate).toHaveCSS("background-color", idle);
    await expect(summarize).toHaveCSS("background-color", idle);
  }
  await page.getByRole("button", { name: "Use light theme" }).click();
  await page.mouse.move(0, 0);
  await toolbar.screenshot({ path: test.info().outputPath("toolbar.png") });
  await page.setViewportSize({ width: 375, height: 812 });
  await page
    .getByRole("heading", {
      name: "Async Rust without the hidden machinery",
      level: 2,
    })
    .click();
  await expect(
    toolbar.getByRole("button", { name: "Back to articles" }),
  ).toBeVisible();
  // The existing mobile reader slides in; measure after that navigation ends.
  await expect.poll(async () => (await toolbar.boundingBox())!.x).toBe(0);
  const mobileBounds = await bounds();
  expect(mobileBounds.every((box) => box.x >= 0 && box.right <= 375)).toBe(
    true,
  );
  await toolbar.screenshot({
    path: test.info().outputPath("toolbar-mobile.png"),
  });
});

test("summary loading preserves every toolbar target and blocks repeat activation", async ({
  page,
}) => {
  let release!: () => void;
  const held = new Promise<void>((resolve) => (release = resolve));
  let requests = 0;
  await page.route("**/api/articles/*/chats?*", async (route) => {
    requests++;
    await held;
    await route.fulfill({ json: [] });
  });
  await page.goto("/reader");
  const toolbar = page.locator(".reader-toolbar");
  const summarize = toolbar.getByRole("button", { name: "Summarize" });
  await expect(summarize).toBeVisible();
  const bounds = () =>
    toolbar
      .locator("button:visible, a:visible")
      .evaluateAll((elements) =>
        elements.map((el) => el.getBoundingClientRect().toJSON()),
      );
  const before = await bounds();
  try {
    await summarize.click();
    await expect(summarize).toHaveAttribute("aria-busy", "true");
    await expect(summarize).toBeDisabled();
    await expect(summarize.locator(".spinner")).toBeVisible();
    expect(await bounds()).toEqual(before);
    await summarize.evaluate((el: HTMLButtonElement) => el.click());
    await expect.poll(() => requests).toBe(1);
  } finally {
    release();
  }
  await expect(summarize).toBeEnabled();
  expect(await bounds()).toEqual(before);
});
