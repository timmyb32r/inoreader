import { assertWire, checkCriticalResponse } from "./decode";
it("rejects incomplete and unknown critical state values without logging content", () => {
  expect(() =>
    assertWire("ArticlePageView", {
      articles: [],
      total: 0,
      unreadTotal: 0,
      newerCursor: null,
      olderCursor: null,
    }),
  ).not.toThrow();
  expect(() =>
    assertWire("ArticlePageView", { articles: [], total: 0 }),
  ).toThrow("Invalid API response");
  expect(() =>
    assertWire("ArticlePageView", {
      articles: [],
      total: 2 ** 54,
      unreadTotal: 0,
    }),
  ).toThrow();
  expect(() =>
    assertWire("ArticleView", { id: "secret payload", fullText: "surprise" }),
  ).toThrow("Invalid API response (ArticleView)");
});

it("validates flattened AI states and rejects malformed lists", () => {
  const job = {
    id: "job",
    workspaceId: "workspace",
    articleId: "article",
    source: "text",
    model: "model",
    status: "queued",
  };
  expect(() => assertWire("ParagraphJob", job)).not.toThrow();
  expect(() =>
    assertWire("ParagraphJob", { ...job, status: "unexpected" }),
  ).toThrow();
  expect(() => assertWire("ParagraphJob", { status: "queued" })).toThrow();
  expect(() => checkCriticalResponse("/api/subscriptions", {})).toThrow();
  expect(() =>
    checkCriticalResponse("/api/articles/a/translations", job, "POST"),
  ).not.toThrow();
  expect(() =>
    checkCriticalResponse("/api/articles/a/translations", job, "GET"),
  ).toThrow();
});
