import { expect, test } from "@playwright/test";

test("real backend and YDB preserve a reader mutation @acceptance",async({page})=>{
  await page.goto("/");
  await expect(page.getByRole("heading",{name:"All articles"})).toBeVisible();
  const first=page.locator(".article-row").first();await first.getByRole("button",{name:/Save article|Remove from saved/}).click();
  await page.reload();await expect(page.locator(".article-row").first()).toBeVisible();
});
