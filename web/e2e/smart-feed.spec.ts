import { expect, test } from "@playwright/test";
const feed = {
  profile: {
    prompt: "Предпочитаю глубокие исследования СУБД",
    revision: "1",
    trainingCount: 139,
  },
  total: 2,
  scored: 1,
  failed: 0,
  nextCursor: "next",
  articles: [
    {
      id: "research",
      title: "Database research",
      excerpt: "Architecture and measurements",
      prediction: {
        score: 9,
        reason: "Содержательное исследование СУБД",
        confidence: "high",
      },
      error: null,
    },
    {
      id: "pending",
      title: "Unknown article",
      excerpt: "",
      prediction: null,
      error: null,
    },
  ],
};
for (const width of [1440, 390]) {
  test(`smart feed keeps controls stable at ${width}px and protects pending requests`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 900 });
    let release!: () => void;
    let delay = false,
      calls = 0;
    const pending = new Promise<void>((resolve) => {
      release = resolve;
    });
    await page.route("**/api/ai/smart-feed?*", async (route) => {
      calls++;
      if (delay) await pending;
      await route.fulfill({ json: feed });
    });
    await page.route("**/api/ai/interests", async (route) => {
      await route.fulfill({
        status: 409,
        json: { error: "Conflicting profile revision" },
      });
    });
    await page.goto("/smart?workspace=ws");
    await expect(page.getByText("9/10")).toBeVisible();
    const next = page.getByRole("button", { name: "Следующие" });
    const before = await next.boundingBox();
    await page
      .getByRole("button", { name: "Профиль интересов", exact: true })
      .click();
    await expect(
      page.getByRole("textbox", { name: "Профиль интересов" }),
    ).toBeVisible();
    expect(await next.boundingBox()).toEqual(before);
    await page
      .getByRole("textbox", { name: "Профиль интересов" })
      .fill("Мой новый профиль");
    await page.getByRole("button", { name: "Сохранить профиль" }).click();
    await expect(
      page.getByRole("region", { name: "Smart feed" }).getByRole("status"),
    ).not.toContainText("Оценено");
    await expect(
      page.getByRole("textbox", { name: "Профиль интересов" }),
    ).toHaveValue("Мой новый профиль");
    expect(await next.boundingBox()).toEqual(before);
    await page.getByRole("button", { name: "Закрыть", exact: true }).click();
    delay = true;
    const refresh = page.getByRole("button", { name: "Обновить", exact: true });
    await refresh.click();
    await expect(refresh).toBeDisabled();
    await expect(refresh).toHaveAttribute("aria-busy", "true");
    await refresh.evaluate((button: HTMLButtonElement) => button.click());
    await expect.poll(() => calls).toBe(2);
    expect(await next.boundingBox()).toEqual(before);
    release();
    await expect(
      page.getByRole("button", { name: "Обновить", exact: true }),
    ).toBeEnabled();
    expect(await next.boundingBox()).toEqual(before);
    await page.getByRole("link", { name: "Database research" }).click();
    await expect(page).toHaveURL(
      /\/reading\?workspace=ws&article=research&from=smart/,
    );
  });
}

for (const width of [1440, 390]) {
  test(`random feed preserves its seed and fits at ${width}px`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 900 });
    const seed = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    const requests: string[] = [];
    await page.route("**/api/ai/random-feed?*", async (route) => {
      requests.push(route.request().url());
      await route.fulfill({ json: feed });
    });
    await page.goto(`/smart?workspace=ws&order=random&seed=${seed}`);
    await expect(
      page.getByRole("heading", { name: "Случайная лента" }),
    ).toBeVisible();
    await expect(page.getByText("9/10")).toBeVisible();
    expect(new URL(requests[0]).searchParams.get("seed")).toBe(seed);
    const header = page.locator(".smart-feed__header");
    expect(
      await header.evaluate(
        (node) =>
          node.scrollWidth <= node.clientWidth &&
          node.scrollHeight <= node.clientHeight,
      ),
    ).toBe(true);
    await page.getByRole("link", { name: "Database research" }).click();
    await expect(page).toHaveURL(new RegExp(`from=random&seed=${seed}`));
  });
}
