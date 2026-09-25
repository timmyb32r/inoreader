import { expect,test } from "@playwright/test";

test("a 401 moves the application to sign in",async({page})=>{
 await page.route("**/api/bootstrap",route=>route.fulfill({status:401,contentType:"application/json",body:JSON.stringify({message:"Session expired"})}));await page.goto("/");await expect(page.getByRole("heading",{name:"Welcome back"})).toBeVisible();
});

test("invitation and reset screens expose correct autofill contracts",async({page})=>{
 await page.goto("/invite?token=fixed");await expect(page.getByLabel("Username")).toHaveAttribute("autocomplete","username");await expect(page.getByLabel("New password",{exact:true})).toHaveAttribute("autocomplete","new-password");await expect(page.getByLabel("Confirm new password")).toHaveAttribute("name","inoreader-new-password-confirmation");
 await page.goto("/reset-password?token=fixed");await expect(page.getByLabel("New password",{exact:true})).toHaveAttribute("autocomplete","new-password");
 await page.goto("/change-password");await expect(page.getByLabel("Current password")).toHaveAttribute("autocomplete","current-password");
});

test("password reset rejects a mismatched confirmation before transport",async({page})=>{
 let requests=0;page.on("request",request=>{if(request.url().includes("/api/auth/password/reset"))requests++});await page.goto("/reset-password?token=fixed");
 await page.getByLabel("New password",{exact:true}).fill("first value");await page.getByLabel("Confirm new password").fill("different value");await page.getByRole("button",{name:"Continue"}).click();
 await expect(page.getByText("New password and confirmation must match.")).toBeVisible();expect(requests).toBe(0);
});
