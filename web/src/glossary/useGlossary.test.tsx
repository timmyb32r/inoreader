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
