import { expect, test } from "@playwright/test";

test("browser tab follows Feed unread count and clears it on Home", async ({
  page,
}) => {
  await page.goto("/reader");
  const feed = page.getByRole("button", { name: /^Feed \(\d+\)$/ });
  await expect(feed).toBeVisible();
  const initial = await feed.innerText();
  await expect(page).toHaveTitle(`${initial} · Reader`);
  await page
    .locator(".reader-toolbar")
    .getByRole("button", { name: "Mark read", exact: true })
    .click();
  await page.getByRole("button", { name: "Без оценки", exact: true }).click();
  await expect(feed).not.toHaveText(initial);
  await expect(page).toHaveTitle(`${await feed.innerText()} · Reader`);
  await page.getByRole("button", { name: "Home", exact: true }).click();
  await expect(page).toHaveTitle("Reader");
});

test("copy entire article gives immediate stable feedback, preserves all text and rejects duplicate clicks", async ({
  page,
}) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: (text: string) => {
          const state = window as any;
          state.copies = [...(state.copies ?? []), text];
          return new Promise<void>((resolve) => {
            state.finishCopy = resolve;
          });
        },
      },
    });
  });
  await page.goto("/reader");
  const reader = page.getByRole("article", { name: "Article reader" });
  const copy = reader.getByRole("button", { name: "Copy full article" });
  await expect(copy).toBeEnabled();
  await expect(copy).toContainText("Copy article");
  const expected = await reader.evaluate((node) =>
    [
      node.querySelector("h1")?.textContent ?? "",
      node.querySelector(".reader-deck")?.textContent ?? "",
      (node.querySelector(".article-content") as HTMLElement).innerText,
    ]
      .filter((value) => value !== "")
      .join("\n\n"),
  );
  const original = reader.getByRole("link", { name: "Open original" });
  const before = {
    copy: await copy.boundingBox(),
    original: await original.boundingBox(),
  };
  await copy.click();
  await expect(copy).toHaveAttribute("aria-busy", "true");
  await expect(copy).toBeDisabled();
  await copy.dispatchEvent("click");
  expect(await page.evaluate(() => (window as any).copies)).toEqual([expected]);
  expect(await copy.boundingBox()).toEqual(before.copy);
  expect(await original.boundingBox()).toEqual(before.original);
  await page.evaluate(() => (window as any).finishCopy());
  await expect(copy).toHaveAttribute("data-copy-state", "copied");
  expect(await copy.boundingBox()).toEqual(before.copy);
});

test("subscription tabs retain totals and geometry while filtering", async ({
  page,
}) => {
  await page.goto("/subscriptions");
  const tabs = page.getByRole("navigation", { name: "Subscription views" });
  const current = tabs.getByRole("button", { name: /^Current \(\d+\)$/ });
  await expect(current).toBeVisible();
  await expect(
    tabs.getByRole("button", { name: /^Needs attention \(\d+\)$/ }),
  ).toBeVisible();
  await expect(
    tabs.getByRole("button", { name: /^Archived \(\d+\)$/ }),
  ).toBeVisible();
  const text = await tabs.innerText();
  const before = await current.boundingBox();
  await page
    .getByRole("searchbox", { name: "Search subscriptions" })
    .fill("no matches here");
  expect(await tabs.innerText()).toBe(text);
  expect(await current.boundingBox()).toEqual(before);
});
