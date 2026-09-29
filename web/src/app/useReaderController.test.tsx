import { act, renderHook, waitFor } from "@testing-library/preact";
import type { ArticlePage } from "../api/client";
import { articles, mockClient } from "../test/mockClient";
import { useReaderController } from "./useReaderController";

function deferred<T>() {
  let resolve!: (value: T) => void, reject!: (e: Error) => void;
  const promise = new Promise<T>((a, b) => {
    resolve = a;
    reject = b;
  });
  return { promise, resolve, reject };
}
const replacement: ArticlePage = {
  articles: [{ ...articles[0], title: "Other workspace", read: false }],
  total: 1,
  unreadTotal: 100,
};
beforeEach(() => history.replaceState({}, "", "/reader"));
for (const operation of ["page", "mutation", "bulk"] as const)
  it(`ignores late ${operation} responses and rollback after workspace replacement`, async () => {
    const client = mockClient(),
      bootstrap = await client.bootstrap(),
      request = deferred<any>(),
      notice = vi.fn();
    const method =
      operation === "page"
        ? "listArticles"
        : operation === "mutation"
          ? "updateArticle"
          : "markAllRead";
    const spy = vi
      .spyOn(client, method)
      .mockImplementation(() => request.promise);
    const { result } = renderHook(() =>
      useReaderController(client, bootstrap, [], notice, vi.fn()),
    );
    act(() => {
      if (operation === "page") void result.current.loadPage("later", null);
      else if (operation === "mutation")
        result.current.update("1", { read: true });
      else result.current.markAllRead();
    });
    await waitFor(() => expect(spy).toHaveBeenCalledTimes(1));
    act(() => result.current.replaceWorkspace("other", replacement));
    await act(async () => {
      if (operation === "page") request.resolve(bootstrap.articlePage);
      else request.reject(new Error("old request failed"));
      await request.promise.catch(() => {});
    });
    expect(result.current.workspaceId).toBe("other");
    expect(result.current.unreadTotal).toBe(100);
    expect(result.current.selected?.title).toBe("Other workspace");
    expect(result.current.selected?.read).toBe(false);
    expect(result.current.view).toBe("feed");
    expect(notice).not.toHaveBeenCalled();
  });
it("deduplicates same-turn mutations and prevents stale article reads from overwriting them", async () => {
  const client = mockClient(),
    bootstrap = await client.bootstrap(),
    read = deferred<any>(),
    write = deferred<any>();
  const get = vi.spyOn(client, "getArticle").mockReturnValue(read.promise);
  const update = vi
    .spyOn(client, "updateArticle")
    .mockReturnValue(write.promise);
  const { result } = renderHook(() =>
    useReaderController(client, bootstrap, [], vi.fn(), vi.fn()),
  );
  await waitFor(() => expect(get).toHaveBeenCalled());
  act(() => {
    result.current.update("1", { read: true });
    result.current.update("1", { read: true });
  });
  await waitFor(() => expect(update).toHaveBeenCalledTimes(1));
  expect(result.current.pendingArticleMutations.has("1:read")).toBe(true);
  await act(async () => {
    read.resolve(articles[0]);
    await read.promise;
  });
  expect(result.current.selected?.read).toBe(true);
  await act(async () => {
    write.resolve({ ...articles[0], read: true });
    await write.promise;
  });
  await waitFor(() =>
    expect(result.current.pendingArticleMutations.size).toBe(0),
  );
});

it("ignores pending failures after the account controller unmounts", async () => {
  const client = mockClient(),
    bootstrap = await client.bootstrap(),
    pending = deferred<any>(),
    notice = vi.fn();
  vi.spyOn(client, "updateArticle").mockReturnValue(pending.promise);
  const { result, unmount } = renderHook(() =>
    useReaderController(client, bootstrap, [], notice, vi.fn()),
  );
  act(() => result.current.update("1", { read: true }));
  await waitFor(() => expect(client.updateArticle).toHaveBeenCalled());
  unmount();
  await act(async () => {
    pending.reject(new Error("old account"));
    await pending.promise.catch(() => {});
  });
  expect(notice).not.toHaveBeenCalled();
});

for (const failure of [false, true])
  it(`keeps pending writes when paging and reconciles ${failure ? "failure" : "success"}`, async () => {
    const client = mockClient(),
      bootstrap = await client.bootstrap();
    const write = deferred<any>();
    vi.spyOn(client, "getArticle").mockResolvedValue(articles[0]);
    vi.spyOn(client, "updateArticle").mockReturnValue(write.promise);
    const list = vi
      .spyOn(client, "listArticles")
      .mockResolvedValue(bootstrap.articlePage);
    const notice = vi.fn();
    const { result } = renderHook(() =>
      useReaderController(client, bootstrap, [], notice, vi.fn()),
    );
    act(() => {
      result.current.update("1", { read: true });
      void result.current.loadPage("later", null);
      void result.current.loadPage("later", null);
    });
    expect(result.current.paging).toBe(true);
    expect(result.current.pendingArticleMutations.has("1:read")).toBe(true);
    expect(list).not.toHaveBeenCalled();
    await act(async () => {
      if (failure) write.reject(new Error("write failed"));
      else write.resolve({ ...articles[0], read: true });
      await write.promise.catch(() => {});
    });
    await waitFor(() => expect(result.current.paging).toBe(false));
    expect(list).toHaveBeenCalledTimes(1);
    expect(notice).toHaveBeenCalledTimes(failure ? 1 : 0);
    expect(result.current.pendingArticleMutations.size).toBe(0);
  });

it("refetches a page whose snapshot overlaps a mutation", async () => {
  const client = mockClient(),
    bootstrap = await client.bootstrap();
  const stale = deferred<ArticlePage>(),
    write = deferred<any>();
  const fresh = { ...bootstrap.articlePage, unreadTotal: 17 };
  const list = vi
    .spyOn(client, "listArticles")
    .mockReturnValueOnce(stale.promise)
    .mockResolvedValue(fresh);
  vi.spyOn(client, "updateArticle").mockReturnValue(write.promise);
  const { result } = renderHook(() =>
    useReaderController(client, bootstrap, [], vi.fn(), vi.fn()),
  );
  act(() => {
    void result.current.loadPage("later", null);
  });
  await waitFor(() => expect(list).toHaveBeenCalledTimes(1));
  act(() => result.current.update("1", { read: true }));
  await act(async () => {
    stale.resolve(bootstrap.articlePage);
    await stale.promise;
  });
  expect(result.current.pendingArticleMutations.has("1:read")).toBe(true);
  expect(result.current.paging).toBe(true);
  await act(async () => {
    write.resolve({ ...articles[0], read: true });
    await write.promise;
  });
  await waitFor(() => expect(result.current.paging).toBe(false));
  expect(list).toHaveBeenCalledTimes(2);
  expect(result.current.unreadTotal).toBe(17);
});

it("retains the account write barrier across workspace replacement", async () => {
  const client = mockClient(),
    bootstrap = await client.bootstrap(),
    write = deferred<any>();
  vi.spyOn(client, "updateArticle").mockReturnValue(write.promise);
  const list = vi.spyOn(client, "listArticles").mockResolvedValue(replacement);
  const { result } = renderHook(() =>
    useReaderController(client, bootstrap, [], vi.fn(), vi.fn()),
  );
  act(() => result.current.update("1", { read: true }));
  await waitFor(() => expect(client.updateArticle).toHaveBeenCalled());
  act(() => result.current.replaceWorkspace("other", replacement));
  act(() => {
    void result.current.loadPage("feed", null);
  });
  expect(list).not.toHaveBeenCalled();
  await act(async () => {
    write.resolve({ ...articles[0], read: true });
    await write.promise;
  });
  await waitFor(() => expect(list).toHaveBeenCalledTimes(1));
  expect(result.current.selected?.read).toBe(false);
});

it("restarts full-text polling when a refreshed page keeps the same selected article", async () => {
  const client = mockClient(),
    bootstrap = await client.bootstrap();
  const old = deferred<any>();
  const get = vi.spyOn(client, "getArticle").mockReturnValue(old.promise);
  const { result } = renderHook(() =>
    useReaderController(client, bootstrap, [], vi.fn(), vi.fn()),
  );
  await waitFor(() => expect(get).toHaveBeenCalledTimes(1));
  vi.spyOn(client, "listArticles").mockResolvedValue(bootstrap.articlePage);
  const fresh = {
    ...bootstrap.articlePage.articles[0],
    title: "Fresh full text",
  };
  get.mockResolvedValue(fresh);
  await act(async () => {
    await result.current.loadPage("feed", null);
  });
  await waitFor(() =>
    expect(result.current.selected?.title).toBe("Fresh full text"),
  );
  await act(async () => {
    old.resolve({ ...fresh, title: "Stale" });
    await old.promise;
  });
  expect(result.current.selected?.title).toBe("Fresh full text");
});

it("fulltext retry deduplicates writes and restarts a stopped content observer", async () => {
  const client = mockClient(),
    bootstrap = await client.bootstrap();
  bootstrap.articlePage.articles = [{ ...articles[0], fullText: "failed" }];
  const request = deferred<void>();
  const refresh = vi
    .spyOn(client, "refreshFullText")
    .mockReturnValue(request.promise);
  const get = vi
    .spyOn(client, "getArticle")
    .mockResolvedValue(bootstrap.articlePage.articles[0]);
  const { result } = renderHook(() =>
    useReaderController(client, bootstrap, [], vi.fn(), vi.fn()),
  );
  await waitFor(() => expect(get).toHaveBeenCalledTimes(1));
  let accepted!: Promise<void>;
  act(() => {
    accepted = result.current.refreshFullText("1");
    expect(result.current.refreshFullText("1")).toBe(accepted);
  });
  expect(result.current.pendingArticleMutations.has("1:fulltext")).toBe(true);
  expect(refresh).toHaveBeenCalledTimes(1);
  get.mockResolvedValue({
    ...articles[0],
    fullText: "ready",
    body: ["Fetched again"],
  });
  await act(async () => {
    request.resolve();
    await accepted;
  });
  await waitFor(() =>
    expect(result.current.selected?.body).toEqual(["Fetched again"]),
  );
  expect(result.current.pendingArticleMutations.size).toBe(0);
});

it("restores article URLs in another owned workspace without mixing subscriptions", async () => {
  const client = mockClient(),
    bootstrap = await client.bootstrap();
  const restored = {
    ...bootstrap,
    activeWorkspaceId: "finance",
    subscriptions: [],
    articlePage: replacement,
  };
  const changed = vi.fn();
  const boot = vi.spyOn(client, "bootstrap").mockResolvedValue(restored);
  const { result } = renderHook(() =>
    useReaderController(
      client,
      bootstrap,
      bootstrap.subscriptions,
      vi.fn(),
      vi.fn(),
      changed,
    ),
  );
  act(() => {
    history.pushState({}, "", "/reader?workspace=finance&article=1");
    window.dispatchEvent(new PopStateEvent("popstate"));
  });
  await waitFor(() => expect(result.current.workspaceId).toBe("finance"));
  expect(boot).toHaveBeenCalledWith(
    expect.objectContaining({ workspaceId: "finance", articleId: "1" }),
  );
  expect(result.current.selected?.title).toBe("Other workspace");
  expect(changed).toHaveBeenCalledWith(restored);
});
