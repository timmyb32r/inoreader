import { expect, test } from "@playwright/test";

test("countdown restores, pauses, completes once and preserves header geometry", async ({
  page,
}) => {
  await page.addInitScript(() => {
    class TestAudio {
      state = "running";
      currentTime = 0;
      destination = {};
      resume() {
        return Promise.resolve();
      }
      close() {
        return Promise.resolve();
      }
      createGain() {
        return {
          connect() {},
          gain: { setValueAtTime() {}, exponentialRampToValueAtTime() {} },
        };
      }
      createOscillator() {
        return {
          connect() {},
          frequency: { value: 0 },
          stop() {},
          start() {
            localStorage.setItem(
              "test:timer-sounds",
              String(
                Number(localStorage.getItem("test:timer-sounds") || 0) + 1,
              ),
            );
          },
        };
      }
    }
    Object.defineProperty(window, "AudioContext", { value: TestAudio });
  });
  await page.clock.install();
  await page.goto("/reader");
  const timer = page.locator(".countdown");
  const theme = page.getByRole("button", { name: "Use dark theme" });
  const before = await theme.boundingBox();
  await page.getByRole("button", { name: /Timer .*Set duration/ }).click();
  await page.getByRole("textbox", { name: "Timer duration" }).fill("00:00:05");
  await page.getByRole("button", { name: "Set duration", exact: true }).click();
  expect(await theme.boundingBox()).toEqual(before);
  await page.getByRole("button", { name: "Start timer", exact: true }).click();
  await expect(page.getByRole("button", { name: "Pause timer" })).toBeVisible();
  await page.clock.runFor(2100);
  await expect(timer).toContainText("00:00:03");
  await page.getByRole("button", { name: "Pause timer" }).click();
  await page.clock.runFor(10000);
  await expect(timer).toContainText("00:00:03");
  await page.reload();
  await expect(
    page.getByRole("button", { name: "Resume timer" }),
  ).toBeVisible();
  await expect(timer).toContainText("00:00:03");
  await page.getByRole("button", { name: "Resume timer" }).click();
  await page.clock.runFor(3500);
  await expect(timer).toHaveClass(/finished/);
  expect(
    await page.evaluate(() => localStorage.getItem("test:timer-sounds")),
  ).toBe("1");
  await expect(timer).toContainText("00:00:00");
  expect(await theme.boundingBox()).toEqual(before);
  await page.reload();
  await expect(timer).toHaveClass(/finished/);
  expect(
    await page.evaluate(() => localStorage.getItem("test:timer-sounds")),
  ).toBe("1");
  await page.getByRole("button", { name: "Start timer", exact: true }).click();
  await page.reload();
  await expect(page.getByRole("button", { name: "Pause timer" })).toBeVisible();
  await page.clock.runFor(6000);
  await expect(timer).toContainText("00:00:00");
  await page.getByRole("button", { name: "Stop timer" }).click();
  await expect(timer).toContainText("00:00:05");
  expect(await theme.boundingBox()).toEqual(before);
  await page.setViewportSize({ width: 390, height: 844 });
  expect(
    await page
      .locator(".topbar")
      .evaluate((el) => el.scrollWidth <= el.clientWidth),
  ).toBe(true);
  await timer.screenshot({ path: "test-results/countdown-mobile.png" });
});

test("tabs share timer state and invalid duration does not alter a session", async ({
  page,
  context,
}) => {
  await page.goto("/reader");
  const other = await context.newPage();
  await other.goto("/reader");
  await page.getByRole("button", { name: "Start timer", exact: true }).click();
  await expect(
    other.getByRole("button", { name: "Pause timer" }),
  ).toBeVisible();
  await other.getByRole("button", { name: "Pause timer" }).click();
  await expect(
    page.getByRole("button", { name: "Resume timer" }),
  ).toBeVisible();
  await other.getByRole("button", { name: "Stop timer" }).click();
  await expect(page.locator(".countdown")).toContainText("01:00:00");
  await page.getByRole("button", { name: /Timer .*Set duration/ }).click();
  const submit = page.getByRole("button", {
    name: "Set duration",
    exact: true,
  });
  const before = await submit.boundingBox();
  await page.getByRole("textbox", { name: "Timer duration" }).fill("00:00:00");
  await submit.click();
  await expect(
    page.getByText("Duration must be greater than zero"),
  ).toBeVisible();
  expect(await submit.boundingBox()).toEqual(before);
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Start timer", exact: true }).click();
  await expect(page.getByRole("button", { name: "Pause timer" })).toBeVisible();
});
