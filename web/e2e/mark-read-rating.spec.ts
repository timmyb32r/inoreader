import { expect, test } from "@playwright/test";

test("ordinary Mark read retains feedback drafts, locks saves, and replays a lost receipt exactly", async ({
  page,
}) => {
  const commands: any[] = [];
  let release!: () => void;
  const gate = new Promise<void>((r) => (release = r));
  let committed = false;
  await page.route("**/api/articles/1/reading?*", async (route) => {
    if (route.request().method() === "GET")
      return route.fulfill({
        json: {
          revision: committed ? "1" : "0",
          read: committed,
          rating: committed ? 8 : null,
          ratedAt: committed ? "2026-09-30T01:00:00Z" : null,
          reason: committed ? commands[0].reason : null,
        },
      });
    const command = route.request().postDataJSON();
    commands.push(command);
    if (commands.length === 1) {
      committed = true;
      await gate;
      return route.abort("connectionreset");
    }
    return route.fulfill({
      json: {
        operationId: command.operationId,
        articleId: "1",
        undone: false,
        state: {
          read: true,
          revision: "1",
          rating: 8,
          ratedAt: "2026-09-30T01:00:00Z",
          reason: command.reason,
        },
      },
    });
  });
  await page.goto("/reader");
  const mark = page.getByRole("button", { name: "Mark read", exact: true });
  const before = await mark.boundingBox();
  const sourceRow = page.locator(".reader-source-row");
  const sourceBounds = await sourceRow.boundingBox();
  const wikiBounds = await page.locator(".reader-wiki-slot").boundingBox();
  await mark.click();
  const dialog = page.getByRole("dialog", {
    name: "Оценить статью",
    exact: true,
  });
  const reason = dialog.getByRole("textbox");
  await expect(reason).toBeHidden();
  await dialog
    .getByRole("button", { name: "Rate 8 out of 10", exact: true })
    .click();
  const text = "  Конкретные замеры; 中文 🦆\nНо нет цены эксплуатации.  ";
  await reason.fill(text);
  await page.keyboard.press("Escape");
  expect(commands).toHaveLength(0);
  expect(await mark.boundingBox()).toEqual(before);
  await mark.click();
  expect(await reason.inputValue()).toBe(text);
  await expect(reason).toHaveValue(text);
  const save = dialog.getByRole("button", { name: "Сохранить", exact: true });
  const bounds = await save.boundingBox();
  await save.click();
  await expect(save).toBeDisabled();
  await expect(save).toHaveAttribute("aria-busy", "true");
  await save.dispatchEvent("click");
  await page.keyboard.press("Escape");
  await expect(dialog).toBeVisible();
  expect(await save.boundingBox()).toEqual(bounds);
  await expect.poll(() => commands.length).toBe(1);
  release();
  await expect(
    dialog.getByRole("button", { name: "Повторить", exact: true }),
  ).toBeEnabled();
  await page.reload();
  await expect(reason).toHaveValue(text);
  await expect(reason).toBeDisabled();
  await dialog.getByRole("button", { name: "Повторить", exact: true }).click();
  await expect(dialog).toBeHidden();
  expect(commands).toHaveLength(2);
  expect(commands[0]).toEqual(commands[1]);
  expect(commands[1].reason).toBe(text);
  await expect(page.getByLabel("Способ чтения")).toHaveText(
    "Прочитано в ридере",
  );
  expect(await sourceRow.boundingBox()).toEqual(sourceBounds);
  expect(await page.locator(".reader-wiki-slot").boundingBox()).toEqual(
    wikiBounds,
  );
  await expect(
    page.getByRole("button", { name: "Mark unread", exact: true }),
  ).toBeVisible();
});

test("optional feedback never discards typed text, and cancel/skip preserve narrow targets", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/reader?article=1&workspace=ws");
  // Direct entry keeps the selected unread article without an automatic row click.
  const mark = page.getByRole("button", { name: "Mark read", exact: true });
  await mark.evaluate((el) => (el as HTMLButtonElement).click());
  const dialog = page.getByRole("dialog", {
    name: "Оценить статью",
    exact: true,
  });
  const skip = dialog.getByRole("button", { name: "Без оценки", exact: true });
  await expect(skip).toBeEnabled();
  const bounds = await skip.boundingBox();
  await expect(dialog.getByRole("textbox")).toBeHidden();
  await dialog.getByRole("button", { name: "Не знаю", exact: true }).click();
  await dialog.getByRole("textbox").fill("Причина без оценки");
  await expect(skip).toBeDisabled();
  expect(await skip.boundingBox()).toEqual(bounds);
  await dialog.getByRole("textbox").fill("");
  await dialog.getByRole("button", { name: "Не знаю", exact: true }).click();
  await expect(skip).toBeEnabled();
  expect(await skip.boundingBox()).toEqual(bounds);
  await skip.click();
  await expect(dialog).toBeHidden();
});
