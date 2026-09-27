import { assertWire, decodeResponse } from "./decode";
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
  expect(() => decodeResponse("SubscriptionView[]", {})).toThrow();
  expect(() => decodeResponse("ParagraphJob", job)).not.toThrow();
  expect(() => decodeResponse("ParagraphJob[]", job)).toThrow();
});

it("rejects wrong no-content and scalar responses", () => {
  expect(() => decodeResponse("empty", {})).toThrow();
  expect(() => decodeResponse("string", {})).toThrow();
  expect(decodeResponse("empty", undefined)).toBeUndefined();
  expect(decodeResponse("string", "exact document")).toBe("exact document");
});
