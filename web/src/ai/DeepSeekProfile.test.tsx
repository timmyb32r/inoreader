import { fireEvent, render, screen, waitFor } from "@testing-library/preact";
import userEvent from "@testing-library/user-event";
import { AiClient, type AiProfile } from "../api/ai";
import type { Transport } from "../api/client";
import { DeepSeekProfile } from "./DeepSeekProfile";

const configured: AiProfile = { configured: true, enabled: true, balance: { available: true, updatedAt: "2026-09-26T20:00:00Z", balances: [{ currency: "USD", total: "12.34567890", granted: "0.0000", toppedUp: "12.34567890" }] } };

it("validates/saves once, clears the key input and preserves exact balance strings", async () => {
  let resolveSave!: (value: AiProfile) => void;
  const calls: { path: string; init?: RequestInit }[] = [];
  const transport: Transport = async <T,>(path: string, init?: RequestInit) => {
    calls.push({ path, init });
    if (init?.method === "PUT") return await new Promise<AiProfile>(resolve => { resolveSave = resolve; }) as T;
    return { configured: false, enabled: false } as T;
  };
  const user = userEvent.setup(), onProfile = vi.fn();
  render(<DeepSeekProfile client={new AiClient(transport)} onProfile={onProfile} completedGeneration={0}/>);
  const input = screen.getByLabelText("DeepSeek API key");
  await waitFor(() => expect(input).toBeEnabled());
  expect(input).toHaveAttribute("type", "password");
  expect(input).toHaveAttribute("autocomplete", "none");
  await user.type(input, "test-secret");
  const button = screen.getByRole("button", { name: "Save API key" });
  fireEvent.click(button); fireEvent.click(button);
  expect(button).toBeDisabled();
  expect(button).toHaveAttribute("aria-busy", "true");
  expect(calls.filter(call => call.init?.method === "PUT")).toHaveLength(1);
  expect(JSON.parse(String(calls.at(-1)?.init?.body))).toEqual({ apiKey: "test-secret" });
  expect(calls.every(call => !call.path.includes("test-secret"))).toBe(true);
  resolveSave(configured);
  await screen.findByText("12.34567890 USD");
  expect(screen.getByLabelText("Replace DeepSeek API key")).toHaveValue("");
  expect(screen.getByText(/Key saved/)).toHaveTextContent("••••••••");
  expect(screen.queryByText("test-secret")).toBeNull();
  expect(onProfile).toHaveBeenLastCalledWith(configured);
});

it("retains the key and last successful balance when a provider refresh fails", async () => {
  const transport: Transport = async <T,>(path: string) => {
    if (path.endsWith("balance")) throw new Error("Provider temporarily unavailable");
    return configured as T;
  };
  render(<DeepSeekProfile client={new AiClient(transport)} onProfile={() => {}} completedGeneration={0}/>);
  await screen.findByText("Provider temporarily unavailable");
  expect(screen.getByText("12.34567890 USD")).toBeVisible();
  expect(screen.getByText(/Key saved/)).toBeVisible();
  expect(screen.getByRole("button", { name: "Remove key" })).toBeEnabled();
});

it("requires explicit removal confirmation and keeps chat data outside credential mutation", async () => {
  const calls: string[] = [];
  const transport: Transport = async <T,>(path: string, init?: RequestInit) => {
    calls.push(`${init?.method ?? "GET"} ${path}`);
    return (init?.method === "DELETE" ? { configured: false, enabled: false } : configured) as T;
  };
  const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
  const user = userEvent.setup();
  render(<DeepSeekProfile client={new AiClient(transport)} onProfile={() => {}} completedGeneration={0}/>);
  const remove = screen.getByRole("button", { name: "Remove key" });
  await waitFor(() => expect(remove).toBeEnabled());
  await user.click(remove);
  expect(calls).not.toContain("DELETE /api/ai/profile");
  confirm.mockReturnValue(true);
  await user.click(remove);
  await screen.findByText("API key removed; your chats are preserved");
  expect(calls.filter(call => call.startsWith("DELETE"))).toEqual(["DELETE /api/ai/profile"]);
  confirm.mockRestore();
});

it("refreshes balance when a generation finishes while the profile is open", async () => {
  let balances = 0;
  const transport: Transport = async <T,>(path: string) => { if (path.endsWith("balance")) balances += 1; return configured as T; };
  const client = new AiClient(transport), onProfile = vi.fn();
  const view = render(<DeepSeekProfile client={client} onProfile={onProfile} completedGeneration={0}/>);
  await waitFor(() => expect(screen.getByRole("button", { name: "Refresh balance" })).toBeEnabled());
  expect(balances).toBe(1);
  view.rerender(<DeepSeekProfile client={client} onProfile={onProfile} completedGeneration={1}/>);
  await waitFor(() => expect(balances).toBe(2));
});
