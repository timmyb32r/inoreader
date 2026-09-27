import { fireEvent, render, screen, waitFor } from "@testing-library/preact";
import { ChatCopyButton } from "./ChatCopyButton";

it("copies exact text with immediate pending feedback, duplicate protection and an icon", async () => {
  let finish!: () => void;
  const copy = vi.fn(() => new Promise<void>(resolve => { finish = resolve; }));
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: copy } });
  render(<ChatCopyButton text={"**Exact summary**\n\n12.5%"} label="Copy response"/>);
  const button = screen.getByRole("button", { name: "Copy response" });
  expect(button.querySelector("svg")).not.toBeNull();
  fireEvent.click(button); fireEvent.click(button);
  expect(button).toBeDisabled();
  expect(button).toHaveAttribute("aria-busy", "true");
  expect(copy).toHaveBeenCalledExactlyOnceWith("**Exact summary**\n\n12.5%");
  finish();
  await waitFor(() => expect(button).toHaveAttribute("data-copy-state", "copied"));
  expect(button).toBeEnabled();
});

it("reports clipboard failures on the same control", async () => {
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: vi.fn().mockRejectedValue(new Error("denied")) } });
  render(<ChatCopyButton text="Exact text" label="Copy response"/>);
  const button = screen.getByRole("button", { name: "Copy response" });
  fireEvent.click(button);
  await waitFor(() => expect(button).toHaveAttribute("data-copy-state", "failed"));
  expect(button).toHaveAttribute("title", "Copy failed — select the text to copy");
});
