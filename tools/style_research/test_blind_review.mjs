import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createRequire } from 'node:module';
import { pathToFileURL, fileURLToPath } from 'node:url';
import test from 'node:test';

const require = createRequire(new URL('../../web/package.json', import.meta.url));
const { chromium } = require('playwright');
const generator = fileURLToPath(new URL('render_blind_review.py', import.meta.url));
let browser, directory, pageUrl;
const evaluationId = '7042c39e-993b-411b-8d04-3e1dd0aeab56';
const storageKey = `blind-author-review:v1:${evaluationId}`;
const dangerousText = '**Exact heading**\n\n<script>window.PWNED=true</script>\n<img src="https://bad.example/pixel">\n[unsafe](javascript:alert(1))\n**Выделение** и `код`\n' + 'Полный текст. '.repeat(1000) + '\nLAST_SENTINEL';

test.before(async () => {
  directory = await mkdtemp(path.join(tmpdir(), 'blind-review-test-'));
  const manifest = {
    schema_version: 1, evaluation_id: evaluationId,
    items: Array.from({ length: 20 }, (_, index) => ({
      id: `e5da-${index}`, title: `Exact title ${index}`,
      url: `https://publisher.example/${index}?a=1&b=2`,
      text: index === 0 ? dangerousText : `**Title ${index}**\n\nShort summary ${index}.`,
    })),
  };
  const input = path.join(directory, 'input.json');
  const output = path.join(directory, 'index.html');
  await writeFile(input, JSON.stringify(manifest));
  execFileSync('python3', [generator, '--manifest', input, '--output', output]);
  pageUrl = pathToFileURL(output).href;
  browser = await chromium.launch({ headless: true });
});
test.after(async () => {
  await browser?.close();
  if (directory) await rm(directory, { recursive: true, force: true });
});

async function openPage(options = {}) {
  const context = await browser.newContext({ acceptDownloads: true, ...options });
  const page = await context.newPage();
  await page.goto(pageUrl);
  await page.locator('#summary').waitFor();
  assert.equal(await page.locator('#item-number').textContent(), 'Материал 1 из 20');
  return { context, page };
}

async function exported(page) {
  const pending = page.waitForEvent('download');
  await page.locator('#export').click();
  const download = await pending;
  return JSON.parse(await readFile(await download.path(), 'utf8'));
}

test('safe complete rendering, no preset vote, no remote requests, full raw text', async () => {
  const context = await browser.newContext();
  const page = await context.newPage();
  const remote = [];
  page.on('request', request => { if (/^https?:/.test(request.url())) remote.push(request.url()); });
  await page.goto(pageUrl);
  assert.equal(await page.locator('#like').getAttribute('aria-pressed'), 'false');
  assert.equal(await page.locator('#unlike').getAttribute('aria-pressed'), 'false');
  assert.equal(await page.locator('#progress-text').textContent(), 'Оценено 0 из 20');
  assert.equal(await page.evaluate(() => window.PWNED), undefined);
  assert.equal(await page.locator('#summary img, #summary script').count(), 0);
  assert.equal(await page.locator('#summary a').count(), 0);
  assert.equal(await page.locator('#summary strong').first().textContent(), 'Exact heading');
  assert.match(await page.locator('#summary').textContent(), /LAST_SENTINEL$/);
  await page.locator('#raw-toggle').click();
  assert.equal(await page.locator('#summary').textContent(), dangerousText);
  assert.equal(await page.locator('#source-link').getAttribute('href'), 'https://publisher.example/0?a=1&b=2');
  assert.equal(await page.locator('#comment').getAttribute('autocomplete'), 'none');
  assert.match(await page.locator('#comment').getAttribute('name'), /^f_[a-f0-9-]+$/);
  assert.deepEqual(remote, []);
  await context.close();
});

test('immediate feedback, stable controls, retained ratings, reload and explicit partial export', async () => {
  const { context, page } = await openPage();
  const before = await page.locator('#next').boundingBox();
  const immediate = await page.evaluate(() => {
    document.querySelector('#like').click();
    return document.querySelector('#like').getAttribute('aria-pressed');
  });
  assert.equal(immediate, 'true');
  assert.deepEqual(await page.locator('#next').boundingBox(), before);
  await page.locator('#comment').fill('Тут не мой оборот. <b>Не HTML</b>');
  await page.locator('#issue-numbers').check();
  await page.locator('#next').click();
  assert.equal(await page.locator('#like').getAttribute('aria-pressed'), 'false');
  await page.locator('#previous').click();
  assert.equal(await page.locator('#like').getAttribute('aria-pressed'), 'true');
  assert.equal(await page.locator('#issue-numbers').isChecked(), true);
  assert.equal(await page.locator('#comment').inputValue(), 'Тут не мой оборот. <b>Не HTML</b>');
  await page.reload();
  assert.equal(await page.locator('#like').getAttribute('aria-pressed'), 'true');
  const result = await exported(page);
  assert.equal(result.status, 'partial');
  assert.equal(result.completed_items, 1);
  assert.equal(result.items.length, 20);
  assert.equal(result.items[0].text, dangerousText);
  assert.equal(result.items[0].issues.numbers, true);
  assert.equal(result.items[1].verdict, null);
  assert.match(await page.locator('#status').textContent(), /Экспорт неполный/);
  assert.deepEqual(await page.locator('#next').boundingBox(), before);
  await context.close();
});

test('complete export has no inferred approvals and rejects duplicate export activation', async () => {
  const { context, page } = await openPage();
  for (let index = 0; index < 20; index++) {
    await page.locator('.item-button').nth(index).click();
    await page.locator(index % 2 ? '#unlike' : '#like').click();
  }
  let downloads = 0;
  page.on('download', () => { downloads += 1; });
  const pending = page.waitForEvent('download');
  const immediate = await page.evaluate(() => {
    const button = document.querySelector('#export'); button.click();
    const state = { disabled: button.disabled, busy: button.getAttribute('aria-busy') };
    button.click(); return state;
  });
  assert.deepEqual(immediate, { disabled: true, busy: 'true' });
  const download = await pending;
  const result = JSON.parse(await readFile(await download.path(), 'utf8'));
  assert.equal(result.status, 'complete');
  assert.equal(result.completed_items, 20);
  assert.equal(result.items.filter(item => item.verdict === 'unlike').length, 10);
  assert.equal(downloads, 1);
  await context.close();
});

test('mobile and dark layout stay within viewport during interaction', async () => {
  const { context, page } = await openPage({ viewport: { width: 375, height: 812 }, colorScheme: 'dark' });
  const targets = ['#like', '#unlike', '#comment', '#next', '#export'];
  const before = await Promise.all(targets.map(selector => page.locator(selector).boundingBox()));
  await page.locator('#unlike').click();
  await page.locator('#issue-facts').check();
  await page.locator('#comment').fill('Не так написал бы.\nЧисло надо пояснить.');
  const after = await Promise.all(targets.map(selector => page.locator(selector).boundingBox()));
  assert.deepEqual(after, before);
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
  for (const box of after) assert.ok(box.x >= 0 && box.x + box.width <= 375 && box.y + box.height <= 812);
  await context.close();
});

test('foreign, corrupt and changed manifests never silently overwrite previous drafts', async () => {
  const { context, page } = await openPage();
  const foreign = 'blind-author-review:v1:other-evaluation';
  await page.evaluate(([ownKey, foreignKey]) => {
    localStorage.setItem(foreignKey, 'FOREIGN_DRAFT');
    const draft = JSON.parse(localStorage.getItem(ownKey));
    draft.manifest.items[0].text = 'Different earlier text';
    localStorage.setItem(ownKey, JSON.stringify(draft));
  }, [storageKey, foreign]);
  const original = await page.evaluate(key => localStorage.getItem(key), storageKey);
  await page.reload();
  assert.match(await page.locator('#status').textContent(), /не совпадает/);
  await page.locator('#unlike').click();
  assert.equal(await page.evaluate(key => localStorage.getItem(key), storageKey), original);
  assert.equal(await page.evaluate(key => localStorage.getItem(key), foreign), 'FOREIGN_DRAFT');
  const result = await exported(page);
  assert.equal(result.items[0].verdict, 'unlike');
  await page.evaluate(key => localStorage.setItem(key, '{not JSON'), storageKey);
  await page.reload();
  await page.locator('#comment').fill('Do not overwrite corrupt bytes');
  assert.equal(await page.evaluate(key => localStorage.getItem(key), storageKey), '{not JSON');
  assert.equal(await page.locator('#recover-draft').isVisible(), true);
  await context.close();
});
