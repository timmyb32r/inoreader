import { act, renderHook } from "@testing-library/preact";
import { GlossaryClient, type DefinitionsView } from "../api/glossary";
import { useGlossary } from "./useGlossary";

const empty: DefinitionsView = {
  channel: {
    channel: "reading_data_news",
    configured: false,
    conflicts: 0,
    coverageNote: "fixture",
    definitions: 1,
    generationAllowed: true,
    historyIncomplete: false,
    indexReady: true,
    pending: 0,
    posts: 1,
    revision: 1,
    syncPending: false,
    unindexed: 0,
  },
  known: [],
};
it("reuses an uncertain paid operation after closing, reopening and retrying", async () => {
  const client = new GlossaryClient(async () => {
    throw new Error("unexpected transport");
  });
  vi.spyOn(client, "get").mockResolvedValue(empty);
  const generate = vi
    .spyOn(client, "generate")
    .mockRejectedValueOnce(new Error("connection lost"))
    .mockResolvedValue(empty);
  const { result } = renderHook(() =>
    useGlossary(client, "account", "workspace"),
  );
  await act(async () => {
    await result.current.open("article", "Title");
  });
  expect(result.current.error).toBe("connection lost");
  const first = generate.mock.calls[0];
  act(() => result.current.close());
  await act(async () => {
    await result.current.open("article", "Title");
  });
  expect(generate.mock.calls[1]).toEqual(first);
  await act(async () => {
    await result.current.retry();
  });
  expect(generate.mock.calls[2][2]).not.toBe(first[2]);
  expect(generate.mock.calls[2][3]).toBe(true);
});

it("loads saved definitions without a paid request when reader opens an archive article", async () => {
  const client = new GlossaryClient(async () => {
    throw new Error("unexpected transport");
  });
  const get = vi.spyOn(client, "get").mockResolvedValue(empty);
  const generate = vi.spyOn(client, "generate").mockResolvedValue(empty);
  const { result } = renderHook(() =>
    useGlossary(client, "account", "workspace"),
  );
  await act(async () => {
    await result.current.open("archive", "Old article", false);
  });
  expect(get).toHaveBeenCalledOnce();
  expect(generate).not.toHaveBeenCalled();
  expect(result.current.view).toEqual(empty);
  await act(async () => {
    await result.current.retry();
  });
  expect(generate).toHaveBeenCalledOnce();
});

it("retains queued automatic definitions on open and promotes the same job only on demand", async () => {
  const client = new GlossaryClient(async () => {
    throw new Error("unexpected transport");
  });
  const queued: DefinitionsView = {
    ...empty,
    job: {
      id: "queued-job",
      workspaceId: "workspace",
      articleId: "article",
      model: "fixture",
      promptVersion: "fixture",
      status: "queued",
      usage: null,
    },
  };
  vi.spyOn(client, "get").mockResolvedValue(queued);
  let finish!: (value: DefinitionsView) => void;
  const generate = vi.spyOn(client, "generate").mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const { result } = renderHook(() =>
    useGlossary(client, "account", "workspace"),
  );
  await act(async () => {
    await result.current.open("article", "Title", false);
  });
  expect(generate).not.toHaveBeenCalled();
  expect(result.current.busy).toBe(true);
  expect(result.current.actionBusy).toBe(false);
  let request!: Promise<void>;
  await act(async () => {
    request = result.current.open("article", "Title");
    await Promise.resolve();
  });
  expect(result.current.actionBusy).toBe(true);
  expect(generate).toHaveBeenCalledOnce();
  await act(async () => {
    await result.current.open("article", "Title");
  });
  expect(generate).toHaveBeenCalledOnce();
  expect(generate.mock.calls[0][3]).toBe(false);
  await act(async () => {
    finish(queued);
    await request;
  });
  expect(result.current.view?.job?.id).toBe("queued-job");
});
