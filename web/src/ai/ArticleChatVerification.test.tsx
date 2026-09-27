import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/preact";
import { AiClient, type ArticleChat, type ChatProviderCall } from "../api/ai";
import type { Transport } from "../api/client";
import { Harness, saved } from "./tests/chatTestSupport";

const call = (
  phase: ChatProviderCall["phase"],
  status: ChatProviderCall["status"],
): ChatProviderCall => ({ id: phase, assistantId: "m1", phase, status });
const pending = (status: "generating" | "verifying"): ArticleChat => ({
  ...saved(),
  status,
  providerCalls:
    status === "verifying"
      ? [call("generating", "completed"), call("verifying", "started")]
      : [call("generating", "started")],
  messages: [
    {
      ...saved().messages[0],
      phase: status,
      status: "streaming",
      content: status === "verifying" ? "Early summary preview" : "",
    },
  ],
});

it("shows the first complete summary while checking and replaces it with the checked response", async () => {
  vi.useFakeTimers();
  try {
    let reads = 0;
    const transport: Transport = async <T,>(
      path: string,
      init?: RequestInit,
    ) => {
      expect(init).toBeUndefined();
      if (path.includes("/articles/")) return [pending("generating")] as T;
      reads++;
      return (
        reads === 1
          ? pending("verifying")
          : {
              ...saved(),
              providerCalls: [
                call("generating", "completed"),
                call("verifying", "completed"),
              ],
            }
      ) as T;
    };
    render(<Harness client={new AiClient(transport)} />);
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Summarize A" }));
    });
    expect(screen.getByRole("status")).toHaveTextContent("Writing summary");
    expect(screen.queryByText("Early summary preview")).toBeNull();
    expect(
      screen.getByRole("button", { name: "Copy response" }),
    ).toBeDisabled();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(screen.getByRole("status")).toHaveTextContent("Checking facts");
    expect(screen.getByRole("status")).toHaveAttribute("aria-busy", "true");
    expect(screen.queryByLabelText("Provider request costs")).toBeNull();
    expect(screen.getByRole("button", { name: "New summary" })).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "Send", exact: true }),
    ).toBeDisabled();
    expect(
      (await screen.findAllByText("Early summary preview"))[0],
    ).toBeVisible();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(screen.getByText("Useful summary")).toBeVisible();
    expect(screen.getByRole("button", { name: "Copy response" })).toBeEnabled();
    expect(screen.getByRole("status")).toHaveAttribute("aria-busy", "false");
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    expect(reads).toBe(2);
  } finally {
    vi.useRealTimers();
  }
});

it("explains verification failure and retries it once while retaining drafts and prior attempts", async () => {
  const failed: ArticleChat = {
    ...pending("verifying"),
    status: "failed",
    error: "Provider timeout",
    messages: [{ ...pending("verifying").messages[0], status: "failed" }],
  };
  let resolve!: (chat: ArticleChat) => void;
  const retries: string[] = [];
  const transport: Transport = async <T,>(path: string, init?: RequestInit) => {
    if (!init) return [failed] as T;
    expect(path).toBe("/api/ai/chats/chat1/retry");
    retries.push(JSON.parse(String(init.body)).operationId);
    return (await new Promise<ArticleChat>((done) => {
      resolve = done;
    })) as T;
  };
  render(<Harness client={new AiClient(transport)} />);
  fireEvent.click(screen.getByRole("button", { name: "Summarize A" }));
  await screen.findByText(/Verification failed/);
  expect(screen.getByRole("status")).toHaveAttribute(
    "title",
    "Provider timeout",
  );
  expect(
    (await screen.findAllByText("Early summary preview"))[0],
  ).toBeVisible();
  expect(screen.queryByLabelText("Waiting for response")).toBeNull();
  fireEvent.input(screen.getByLabelText("Message DeepSeek"), {
    target: { value: "Keep my question" },
  });
  expect(
    screen.getByRole("button", { name: "Send", exact: true }),
  ).toBeDisabled();
  fireEvent.keyDown(screen.getByLabelText("Message DeepSeek"), {
    key: "Enter",
    ctrlKey: true,
  });
  expect(retries).toHaveLength(0);
  const retry = screen.getByRole("button", { name: "Retry", exact: true });
  fireEvent.click(retry);
  fireEvent.click(retry);
  expect(screen.getByRole("status")).toHaveTextContent("Retrying verification");
  expect(retry).toBeDisabled();
  expect(retries).toHaveLength(1);
  resolve({
    ...pending("verifying"),
    messages: [
      ...failed.messages,
      { ...pending("verifying").messages[0], id: "retry" },
    ],
  });
  await waitFor(() =>
    expect(screen.getByRole("status")).toHaveTextContent("Checking facts"),
  );
  await waitFor(() =>
    expect(
      screen.getAllByRole("region", { name: "DeepSeek response" }),
    ).toHaveLength(2),
  );
  expect(screen.getByLabelText("Message DeepSeek")).toHaveValue(
    "Keep my question",
  );
});

it("cancels verification with immediate feedback and keeps the early summary", async () => {
  let resolve!: (chat: ArticleChat) => void;
  let stops = 0;
  const transport: Transport = async <T,>(path: string, init?: RequestInit) => {
    if (!init) return [pending("verifying")] as T;
    expect(path).toBe("/api/ai/chats/chat1/stop");
    stops++;
    return (await new Promise<ArticleChat>((done) => {
      resolve = done;
    })) as T;
  };
  render(<Harness client={new AiClient(transport)} />);
  fireEvent.click(screen.getByRole("button", { name: "Summarize A" }));
  await waitFor(() =>
    expect(screen.getByRole("status")).toHaveTextContent("Checking facts"),
  );
  const stop = screen.getByRole("button", { name: "Stop", exact: true });
  fireEvent.click(stop);
  fireEvent.click(stop);
  expect(screen.getByRole("status")).toHaveTextContent("Stopping");
  expect(stop).toBeDisabled();
  expect(stops).toBe(1);
  resolve({
    ...pending("verifying"),
    status: "cancelled",
    messages: [{ ...pending("verifying").messages[0], status: "interrupted" }],
  });
  await screen.findByText(/Stopped during verification/);
  expect(screen.getByRole("status")).toHaveAttribute("aria-busy", "false");
  expect(
    (await screen.findAllByText("Early summary preview"))[0],
  ).toBeVisible();
  expect(screen.queryByLabelText("Waiting for response")).toBeNull();
  fireEvent.input(screen.getByLabelText("Message DeepSeek"), {
    target: { value: "Unsent question" },
  });
  expect(
    screen.getByRole("button", { name: "Send", exact: true }),
  ).toBeDisabled();
});

it("continues to show follow-up streaming while retaining the previous verified summary", async () => {
  const chat: ArticleChat = {
    ...saved(),
    status: "generating",
    messages: [
      ...saved().messages,
      {
        id: "followup",
        role: "assistant",
        purpose: "chat",
        phase: "generating",
        status: "streaming",
        content: "Here is the explanation",
        createdAt: saved().createdAt,
      },
    ],
  };
  const transport: Transport = async <T,>() => [chat] as T;
  render(<Harness client={new AiClient(transport)} />);
  fireEvent.click(screen.getByRole("button", { name: "Summarize A" }));
  await screen.findByText("Here is the explanation");
  expect(screen.getByText("Useful summary")).toBeVisible();
  expect(screen.getByRole("status")).toHaveTextContent("DeepSeek is writing");
});
