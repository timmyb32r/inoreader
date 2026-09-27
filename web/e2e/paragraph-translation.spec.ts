import {expect,test} from "@playwright/test";
import type {ParagraphJob} from "../src/api/ai";

test("paragraph translation preserves article geometry and links, deduplicates clicks, and shows instant dictionary cards",async({page})=>{
 const article={id:"a",title:"Column storage",url:"https://example.test/a",source:"Example",excerpt:"",body:[],bodyHtml:'<p>磁<strong>盘</strong> 读取。</p><p>下一段保持原位。</p><p><a href="https://example.test/source">来源</a></p>',fullText:"ready",age:"2026-09-27T10:00:00Z",read:false,later:false};
 const result={source:"磁盘 读取。",translation:"Чтение с диска.",segments:[{kind:"word" as const,source:"磁盘",pinyin:"cípán",translation:"диск"},{kind:"literal" as const,source:" "},{kind:"word" as const,source:"读取",pinyin:"dúqǔ",translation:"читать"},{kind:"literal" as const,source:"。"}]};
 let jobs:ParagraphJob[]=[],posts=0,done=false;
 const base={id:"job",workspaceId:"ws",articleId:"a",source:result.source,model:"deepseek-flash"};
 await page.route("**/api/**",async route=>{
   const path=new URL(route.request().url()).pathname;
   if(path==="/api/bootstrap")return route.fulfill({json:{account:{id:"owner",displayName:"Author",initials:"AU"},workspaces:[{id:"ws",name:"Personal",archived:false}],activeWorkspaceId:"ws",subscriptions:[],articlePage:{articles:[article],total:1,unreadTotal:1}}});
   if(path==="/api/articles/a"||path.endsWith("/state"))return route.fulfill({json:article});
   if(path==="/api/articles")return route.fulfill({json:{articles:[article],total:1,unreadTotal:1}});
   if(path==="/api/ai/profile")return route.fulfill({json:{configured:true,enabled:true}});
   if(path.endsWith("/translations")){
     if(route.request().method()==="POST"){posts++;expect(route.request().postDataJSON().source).toBe(result.source);jobs=[{...base,status:"generating"}];return route.fulfill({json:jobs[0]});}
     return route.fulfill({json:done?[{...base,status:"completed",result}]:jobs});
   }
   return route.fulfill({json:[]});
 });
 await page.goto("/reader");
 const content=page.locator('.article-content'),paragraph=content.locator('p').first(),following=content.locator('p').nth(1),button=page.getByRole('button',{name:'Translate paragraphs',exact:true});
 const before=await following.boundingBox(),toolbar=await page.locator('.reader-toolbar').boundingBox(),text=await content.textContent();
 await button.click();await expect(button).toHaveAttribute('aria-pressed','true');await expect(paragraph).toHaveAttribute('data-translatable','true');
 await paragraph.dblclick();await expect(page.getByRole('region',{name:'Paragraph translation'})).toHaveAttribute('aria-busy','true');expect(posts).toBe(1);
 // Double-click can select browser text; annotations must wait until it clears.
 await page.evaluate(()=>window.getSelection()?.removeAllRanges());done=true;
 await expect(content.locator('[data-translation-word="0"]')).toHaveCount(2);
 expect(await content.textContent()).toBe(text);expect(await following.boundingBox()).toEqual(before);expect(await page.locator('.reader-toolbar').boundingBox()).toEqual(toolbar);
 const span=content.locator('[data-translation-word="0"]').first();await span.hover();
 expect(await page.getByRole('tooltip').isVisible()).toBe(true);await expect(page.getByRole('tooltip')).toContainText('cípán');await expect(page.getByRole('tooltip')).toContainText('диск');
 await content.locator('[data-translation-word="1"]').hover();await expect(page.getByRole('tooltip')).toContainText('dúqǔ');expect(posts).toBe(1);
 await expect(content.getByRole('link',{name:'来源'})).toHaveAttribute('href','https://example.test/source');
 await page.screenshot({path:'/tmp/inoreader-paragraph-translation.png',fullPage:true});
 await page.keyboard.press('Escape');await expect(page.getByRole('tooltip')).toHaveCount(0);
 await button.click();await expect(content.locator('[data-translation-word]')).toHaveCount(0);expect(await following.boundingBox()).toEqual(before);
});
