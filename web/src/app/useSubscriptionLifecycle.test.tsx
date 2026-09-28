import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/preact";
import { ApiClient } from "../api/client";
import { useSubscriptionLifecycle } from "./useSubscriptionLifecycle";

const subscription = {
  id: "one",
  name: "Source",
  count: 1,
  status: "active" as const,
  lastUpdate: "now",
};
function Harness({
  client,
  removed = () => {},
}: {
  client: ApiClient;
  removed?: (id: string) => void;
}) {
  const lifecycle = useSubscriptionLifecycle(client, undefined, removed);
  return (
    <>
      <button onClick={() => lifecycle.open("delete", [subscription])}>
        Delete source
      </button>
      {lifecycle.dialog}
    </>
  );
}
it("requires confirmation, locks immediately and keeps the dialog after deletion", async () => {
  let finish!: () => void;
  const client = new ApiClient(async () => {
    throw new Error("Unexpected request");
  });
  const remove = vi.spyOn(client, "deleteSubscription").mockImplementation(
    () =>
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
  );
  const removed = vi.fn();
  render(<Harness client={client} removed={removed} />);
  fireEvent.click(screen.getByText("Delete source"));
  expect(remove).not.toHaveBeenCalled();
  expect(
    screen.getByText(/Existing articles and reading state stay/),
  ).toBeVisible();
  const confirm = screen.getByRole("button", { name: "Confirm delete" });
  act(() => {
    fireEvent.click(confirm);
    fireEvent.click(confirm);
  });
  expect(remove).toHaveBeenCalledTimes(1);
  expect(confirm).toBeDisabled();
  expect(confirm).toHaveAttribute("aria-busy", "true");
  await act(async () => finish());
  expect(removed).toHaveBeenCalledWith("one");
  expect(screen.getByRole("dialog")).toBeVisible();
  expect(confirm).toBeDisabled();
  expect(
    screen.getByText("Delete complete. Articles preserved."),
  ).toBeVisible();
});
it("reports a failure without removing the subscription and allows retry", async () => {
  const client = new ApiClient(async () => {
    throw new Error("Unexpected request");
  });
  const remove = vi
    .spyOn(client, "deleteSubscription")
    .mockRejectedValueOnce(new Error("Unavailable"))
    .mockResolvedValueOnce(undefined);
  const removed = vi.fn();
  render(<Harness client={client} removed={removed} />);
  fireEvent.click(screen.getByText("Delete source"));
  fireEvent.click(screen.getByText("Confirm delete"));
  await screen.findByText("Source: Unavailable");
  expect(removed).not.toHaveBeenCalled();
  fireEvent.click(screen.getByText("Confirm delete"));
  await waitFor(() => expect(removed).toHaveBeenCalledWith("one"));
  expect(remove).toHaveBeenCalledTimes(2);
});
