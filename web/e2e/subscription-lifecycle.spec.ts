import { expect, test } from "@playwright/test";

test("deletion preserves articles and keeps confirmation controls fixed throughout the request", async ({
  page,
}) => {
  let release!: () => void;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  let calls = 0;
  await page.route("**/api/subscriptions/sub/delete", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    calls++;
    await gate;
    await route.fulfill({ status: 204 });
  });
  await page.goto("/reader");
  const articleTitle = "Async Rust without the hidden machinery";
  await expect(
    page.getByRole("heading", { name: articleTitle, level: 2 }),
  ).toBeVisible();
  await page.getByRole("button", { name: /^Subscriptions \d+$/ }).click();
  await page
    .getByRole("checkbox", { name: "Select This Week in Rust", exact: true })
    .check();
  await page
    .getByRole("button", { name: "Delete This Week in Rust", exact: true })
    .click();
  const dialog = page.getByRole("dialog", {
    name: "Delete subscription",
    exact: true,
  });
  await expect(dialog).toContainText(
    "Existing articles and reading state stay",
  );
  expect(calls).toBe(0);
  const confirm = dialog.getByRole("button", { name: "Confirm delete" });
  const close = dialog.getByRole("button", { name: "Close", exact: true });
  const before = {
    confirm: await confirm.boundingBox(),
    close: await close.boundingBox(),
  };
  await confirm.dblclick();
  await expect(confirm).toHaveAttribute("aria-busy", "true");
  await expect(close).toBeDisabled();
  expect(await confirm.boundingBox()).toEqual(before.confirm);
  expect(await close.boundingBox()).toEqual(before.close);
  expect(calls).toBe(1);
  release();
  await expect(dialog).toContainText("Delete complete. Articles preserved.");
  await expect(confirm).toBeDisabled();
  expect(await confirm.boundingBox()).toEqual(before.confirm);
  expect(await close.boundingBox()).toEqual(before.close);
  await close.click();
  await expect(page.locator(".bulk-bar")).not.toContainText("selected");
  await expect(
    page.getByRole("link", { name: "This Week in Rust", exact: true }),
  ).toHaveCount(0);
  await page
    .getByRole("button", { name: "Close subscriptions", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: articleTitle, level: 2 }),
  ).toBeVisible();
});
