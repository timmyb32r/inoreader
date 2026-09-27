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
