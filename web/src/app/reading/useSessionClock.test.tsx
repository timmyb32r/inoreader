import { act, renderHook, waitFor } from "@testing-library/preact";
import { sessionUrl } from "./readingSession";
import { useSessionClock } from "./useSessionClock";
beforeEach(() => {
  sessionStorage.clear();
  history.replaceState({}, "", "/");
});
afterEach(() => vi.restoreAllMocks());
it("keeps account-owned time away and never exposes the preceding account on a switch", async () => {
  const url =
    sessionUrl("w", { smart: 1, random: 1 }) + "&article=private-news";
  history.replaceState({}, "", url);
  vi.spyOn(Date, "now").mockReturnValue(1000);
  const hook = renderHook(
    ({ owner, route }) => useSessionClock(owner, ["w"], route),
    { initialProps: { owner: "one", route: url } },
  );
  await waitFor(() =>
    expect(hook.result.current.active?.progress.deadline).toBe(61000),
  );
  const pointer = JSON.parse(sessionStorage.getItem("reading-active:one")!);
  expect(JSON.parse(sessionStorage.getItem(pointer)!).url).toContain(
    "private-news",
  );
  history.replaceState({}, "", "/");
  act(() => hook.rerender({ owner: "two", route: "/" }));
  expect(hook.result.current.active).toBeNull();
  expect(sessionStorage.getItem("reading-active:two")).toBeNull();
  expect(sessionStorage.getItem(pointer)).toContain("private-news");
  act(() => {
    hook.result.current.toggle();
  });
  expect(sessionStorage.getItem("reading-active:one")).toBe(
    JSON.stringify(pointer),
  );
});
it("fails closed on a foreign storage pointer without reading or overwriting it", async () => {
  const raw = JSON.stringify("reading-session:other:w:secret");
  sessionStorage.setItem("reading-active:one", raw);
  const get = vi.spyOn(Storage.prototype, "getItem");
  const { result } = renderHook(() => useSessionClock("one", ["w"], "/"));
  await waitFor(() => expect(result.current.error).toContain("повреждён"));
  expect(
    get.mock.calls.some(([key]) => key === "reading-session:other:w:secret"),
  ).toBe(false);
  expect(sessionStorage.getItem("reading-active:one")).toBe(raw);
});
it("stops explicitly while retaining the terminal session record", async () => {
  const url = sessionUrl("w", { smart: 1, random: 1 });
  history.replaceState({}, "", url);
  const { result } = renderHook(() => useSessionClock("one", ["w"], url));
  await waitFor(() => expect(result.current.active).not.toBeNull());
  const pointer = JSON.parse(sessionStorage.getItem("reading-active:one")!);
  act(() => result.current.stop());
  expect(result.current.active).toBeNull();
  expect(sessionStorage.getItem("reading-active:one")).toBeNull();
  expect(JSON.parse(sessionStorage.getItem(pointer)!).progress).toMatchObject({
    finished: true,
    stopped: true,
    remainingMs: 0,
  });
});
