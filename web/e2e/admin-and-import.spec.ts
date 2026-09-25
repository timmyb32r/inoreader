import { expect,test } from "@playwright/test";

test.beforeEach(async({page})=>{await page.goto("/");await expect(page.getByRole("heading",{name:"All articles"})).toBeVisible();});

test("workspace switch reloads its library without moving the picker",async({page})=>{
 const picker=page.getByRole("button",{name:/Data engineering/});const before=await picker.boundingBox();await picker.click();await page.getByRole("button",{name:/Финансы/}).click();
 await expect(page.getByRole("heading",{name:"Finance workspace article",level:1})).toBeVisible();expect(await page.getByRole("button",{name:/Финансы/}).boundingBox()).toEqual(before);
});

test("OPML preview gates apply and export is a download",async({page})=>{
 await page.getByRole("button",{name:"Settings & shortcuts"}).click();const apply=page.getByRole("button",{name:"Apply previewed import"});await expect(apply).toBeDisabled();
 await page.getByLabel("OPML document").fill("<opml version=\"2.0\"/>");await page.getByRole("button",{name:"Preview import"}).dblclick();await expect(page.getByText(/2 subscriptions ready/)).toBeVisible();await expect(apply).toBeEnabled();await apply.click();await expect(page.getByText("Import applied")).toBeVisible();
 const download=page.waitForEvent("download");await page.getByRole("button",{name:"Download OPML export"}).click();expect((await download).suggestedFilename()).toContain("Data engineering");
});

test("administrator creates invite and reset links with one request per activation",async({page})=>{
 await page.getByRole("button",{name:"Settings & shortcuts"}).click();await page.getByLabel("Username").fill("friend");
 await page.getByRole("button",{name:"Create invitation"}).dblclick();await expect(page.getByText(/Invitation: .*token=fixed/)).toBeVisible();
 await page.getByRole("button",{name:"Create password reset"}).click();await expect(page.getByText(/Password reset: .*token=fixed/)).toBeVisible();
});

test("rule preview exposes shared scope and bulk apply reports progress",async({page})=>{
 await page.getByRole("navigation",{name:"Subscriptions"}).getByRole("button",{name:/This Week in Rust/}).click();
 await page.getByRole("button",{name:"Rules"}).click();await page.getByLabel("New literal phrase").fill("career");await page.getByLabel("Match field").selectOption("title");await page.getByLabel("Action").selectOption("move_to_trash");await page.getByRole("button",{name:"Preview matches"}).click();
 const stats=page.locator(".rule-stats");await expect(stats.getByText("4",{exact:true})).toBeVisible();await expect(stats.getByText("2",{exact:true})).toBeVisible();await expect(page.getByText(/shared with linked subscriptions/)).toBeVisible();await page.getByText("Sample article IDs (2)").click();await expect(page.getByText("article-1")).toBeVisible();
 await page.getByRole("button",{name:"Save & apply existing"}).dblclick();await expect(page.getByText(/v1 · career/)).toBeVisible();await expect(page.getByText("Bulk apply: completed")).toBeVisible();await expect(page.getByText("9 articles evaluated")).toBeVisible();page.once("dialog",dialog=>dialog.accept());await page.getByRole("button",{name:"Delete career"}).click();await expect(page.getByText(/v1 · career/)).toHaveCount(0);
});

test("Web Feed stages an advanced recipe, keeps actions stable and requires a fresh preview",async({page})=>{
 const requests:Record<string,unknown>[]=[];page.on("request",request=>{if(request.url().endsWith("/api/web-feeds/recipes"))requests.push(request.postDataJSON())});
 await page.getByRole("button",{name:"Add subscription"}).click();await page.getByRole("button",{name:"Build a Web feed"}).click();const preview=page.getByRole("button",{name:"Preview on server"});const create=page.getByRole("button",{name:"Create Web feed"});const before=await create.boundingBox();
 await page.getByLabel("Public page URL").fill("https://example.test/news");await page.getByRole("textbox",{name:"Link selector",exact:true}).fill("//article");await page.getByLabel("Link selector language").selectOption("xpath");await page.getByLabel("Loading mode").selectOption("browser");await page.getByLabel("Viewport").selectOption("mobile");await page.locator("summary").filter({hasText:"Browser actions"}).click();
 await page.getByRole("button",{name:"Add overlay selector"}).click();await page.getByRole("textbox",{name:"Overlay 1",exact:true}).fill(".cookie-wall");await page.getByRole("button",{name:"Add start page"}).click();await page.getByRole("textbox",{name:"Start page 1",exact:true}).fill("https://example.test/archive");await page.getByRole("textbox",{name:"Load more selector",exact:true}).fill("button.more");await page.getByLabel("Load more clicks").fill("2");await page.getByLabel("Full-page scrolls").fill("1");
 await preview.dblclick();await expect(page.getByText("Selected from live DOM")).toBeVisible();expect(requests).toHaveLength(1);expect(requests[0]).toMatchObject({selectorLanguage:"xpath",viewport:"mobile",hideOverlays:[{language:"css",expression:".cookie-wall"}],startPages:["https://example.test/archive"],loadMoreClicks:2,scrolls:1,preview:true});await expect(create).toBeEnabled();expect(await create.boundingBox()).toEqual(before);
 await page.getByRole("textbox",{name:"Link selector",exact:true}).fill("//article[contains(@class,'story')]");await expect(create).toBeDisabled();await expect(page.getByText("Recipe changed")).toBeVisible();await preview.click();await expect(page.getByText("Selected from live DOM")).toBeVisible();await create.click();await expect(page.getByText("Web feed created")).toBeVisible();
});

test("focus and theme remain stable",async({page})=>{
 await page.getByRole("button",{name:"Use dark theme"}).click();await expect(page.locator(".app")).toHaveAttribute("data-theme","dark");
 await page.getByRole("button",{name:"Settings & shortcuts"}).click();await expect(page.getByRole("dialog")).toBeFocused();await page.getByRole("button",{name:"Close dialog"}).click();
});
