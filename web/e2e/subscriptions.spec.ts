import { expect, test } from "@playwright/test";

test("subscription catalog navigates to durable details and explicitly saves a note", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Subscriptions" }).click();
  await expect(page).toHaveURL(/\/subscriptions$/);
  await expect(page.getByRole("heading", { name: "Subscriptions" })).toBeVisible();

  const search=page.getByRole("searchbox", { name: "Search subscriptions" });
  await search.fill("This Week");
  await expect(page.getByRole("link", { name: "This Week in Rust" })).toBeVisible();
  await search.fill("not present");
  await expect(page.getByText("No subscriptions in this view")).toBeVisible();
  await search.fill("");

  await Promise.all([
    page.waitForResponse(response=>response.url().endsWith("/api/subscriptions/sub")&&response.request().method()==="GET"),
    page.getByRole("link", { name: "This Week in Rust" }).click(),
  ]);
  await expect(page).toHaveURL(/\/subscriptions\/sub$/);
  const note=page.getByRole("textbox", { name: "Personal note" });
  const savedNote=`${await note.inputValue()} Follow for Rust ecosystem changes`.trim();
  await note.fill(savedNote);
  await expect(page.getByText("Unsaved changes")).toBeVisible();
  const save=page.getByRole("button", { name: "Save note" });
  await expect(save).toBeEnabled();
  await save.click();
  await expect(page.getByText("All changes saved")).toBeVisible();

  await page.reload();
  await expect(page.getByRole("textbox", { name: "Personal note" })).toHaveValue(savedNote);
});
