import { act, renderHook, waitFor } from "@testing-library/preact";
import { mockClient } from "../test/mockClient";
import { useSubscriptionCommands } from "./useSubscriptionCommands";

function deferred() {
  let resolve!: () => void, reject!: (error: Error) => void;
  const promise = new Promise<void>((a, b) => {
    resolve = a;
    reject = b;
  });
  return { promise, resolve, reject };
}

it("publishes pending synchronously, deduplicates commands and reports failure to the initiating control", async () => {
  const client = mockClient(),
    request = deferred(),
    changed = vi.fn();
  const resume = vi
    .spyOn(client, "resumeSubscription")
    .mockReturnValue(request.promise);
  const { result } = renderHook(() =>
    useSubscriptionCommands(client, "owner", "ws", changed),
  );
  let first!: Promise<boolean>, second!: Promise<boolean>;
  act(() => {
    first = result.current.resume("sub");
    second = result.current.resume("sub");
  });
  expect(first).toBe(second);
  expect(result.current.pending).toBe(true);
  await waitFor(() => expect(resume).toHaveBeenCalledTimes(1));
  const failed = expect(first).rejects.toThrow("Unavailable");
  await act(async () => request.reject(new Error("Unavailable")));
  await failed;
  expect(changed).not.toHaveBeenCalled();
  await waitFor(() => expect(result.current.pending).toBe(false));
});

for (const change of ["workspace", "account", "unmount"] as const)
  it(`does not publish a late command after ${change}`, async () => {
    const client = mockClient(),
      request = deferred(),
      changed = vi.fn();
    vi.spyOn(client, "resumeSubscription").mockReturnValue(request.promise);
    const { result, rerender, unmount } = renderHook(
      ({ account, workspace }) =>
        useSubscriptionCommands(client, account, workspace, changed),
      { initialProps: { account: "owner", workspace: "ws" } },
    );
    let command!: Promise<boolean>;
    act(() => {
      command = result.current.resume("sub");
    });
    if (change === "unmount") unmount();
    else
      rerender({
        account: change === "account" ? "other" : "owner",
        workspace: change === "workspace" ? "other" : "ws",
      });
    await act(async () => {
      request.resolve();
      await command;
    });
    expect(changed).not.toHaveBeenCalled();
  });
