import { fireEvent, render, screen, waitFor } from "@testing-library/preact";
import { AsyncButton } from "./AsyncButton";
it("shows pending synchronously, preserves label footprint, deduplicates and reports failure", async () => {
  let reject!: (e: Error) => void;
  const promise = new Promise((_, fail) => (reject = fail)),
    press = vi.fn(() => promise),
    error = vi.fn();
  render(
    <AsyncButton onPress={press} onError={error}>
      Run
    </AsyncButton>,
  );
  const button = screen.getByRole("button");
  fireEvent.click(button);
  fireEvent.click(button);
  expect(button).toHaveAttribute("aria-busy", "true");
  expect(button).toHaveAccessibleName("Run");
  expect(button).toBeDisabled();
  expect(button).toHaveTextContent("Run");
  await waitFor(() => expect(press).toHaveBeenCalledTimes(1));
  reject(new Error("failed"));
  await waitFor(() => expect(error).toHaveBeenCalledTimes(1));
  await waitFor(() => expect(button).not.toBeDisabled());
});
