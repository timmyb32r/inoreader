import { expect, test } from "@playwright/test";
test("daily reading activity validates timezone and isolates account data", async ({
  context,
  baseURL,
}) => {
  const token = process.env.READER_ACCEPTANCE_TOKEN;
  if (!token || !baseURL) throw new Error("Missing hermetic fixture");
  await context.addCookies([
    {
      name: "reader_session",
      value: token,
      url: baseURL,
      httpOnly: true,
      sameSite: "Strict",
    },
  ]);
  const bootstrap = await (await context.request.get("/api/bootstrap")).json();
  const workspace = bootstrap.activeWorkspaceId;
  const url = `/api/workspaces/${workspace}/reading-activity?timezone=Europe%2FMoscow`;
  expect(
    (
      await context.request.get(
        `/api/workspaces/${process.env.READER_PRIVATE_WORKSPACE}/reading-activity?timezone=UTC`,
      )
    ).status(),
  ).toBe(404);
  expect(
    (
      await context.request.get(
        `/api/workspaces/${workspace}/reading-activity?timezone=invalid`,
      )
    ).status(),
  ).toBe(422);
  const activity = await context.request.get(url);
  expect(activity.status()).toBe(200);
  expect(activity.headers()["cache-control"]).toBe("no-store");
  const days = (await activity.json()).days;
  expect(Array.isArray(days)).toBe(true);
});

test("source analytics protects private workspaces and validates arbitrary calendar ranges", async ({
  context,
  baseURL,
}) => {
  const token = process.env.READER_ACCEPTANCE_TOKEN;
  if (!token || !baseURL) throw new Error("Missing hermetic fixture");
  await context.addCookies([
    {
      name: "reader_session",
      value: token,
      url: baseURL,
      httpOnly: true,
      sameSite: "Strict",
    },
  ]);
  const bootstrap = await (await context.request.get("/api/bootstrap")).json();
  expect(
    (
      await context.request.get(
        `/api/bootstrap?workspace_id=${process.env.READER_PRIVATE_WORKSPACE}`,
      )
    ).status(),
  ).toBe(404);
  const scoped = await context.request.get(
    `/api/bootstrap?workspace_id=${bootstrap.activeWorkspaceId}`,
  );
  expect(scoped.status()).toBe(200);
  expect((await scoped.json()).activeWorkspaceId).toBe(
    bootstrap.activeWorkspaceId,
  );
  const endpoint = `/api/workspaces/${bootstrap.activeWorkspaceId}/source-activity`;
  const query = "timezone=Europe%2FMoscow&from=2000-01-01&until=2030-01-01";
  const response = await context.request.get(`${endpoint}?${query}`);
  expect(response.status()).toBe(200);
  expect(response.headers()["cache-control"]).toBe("no-store");
  expect(Array.isArray((await response.json()).days)).toBe(true);
  expect(
    (
      await context.request.get(
        `/api/workspaces/${process.env.READER_PRIVATE_WORKSPACE}/source-activity?${query}`,
      )
    ).status(),
  ).toBe(404);
  for (const invalid of [
    "timezone=invalid&from=2026-09-01&until=2026-09-02",
    "timezone=UTC&from=2026-09-02&until=2026-09-01",
    "timezone=UTC&from=2026-02-30&until=2026-09-01",
  ])
    expect(
      (await context.request.get(`${endpoint}?${invalid}`)).status(),
    ).toBeGreaterThanOrEqual(400);
});
