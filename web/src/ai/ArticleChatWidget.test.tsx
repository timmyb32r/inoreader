import type { ResponseContract, ResponseValue } from "../api/decode";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/preact";
import userEvent from "@testing-library/user-event";
import { AiClient, type ArticleChat } from "../api/ai";
import { ApiError, type Transport } from "../api/client";
import { Harness, saved } from "./tests/chatTestSupport";

it("never generates on mount or when reopening an existing saved conversation", async () => {
  const calls: string[] = [];
  const transport: Transport = async <C extends ResponseContract>(
    path: string,
    init?: RequestInit,
  ) => {
    calls.push(`${init?.method ?? "GET"} ${path}`);
    return [saved()] as unknown as ResponseValue<C>;
  };
  const user = userEvent.setup();
  render(<Harness client={new AiClient(transport)} />);
  expect(calls).toEqual([]);
  await user.click(screen.getByRole("button", { name: "Summarize A" }));
  await screen.findByText("Useful summary");
  await user.click(screen.getByRole("button", { name: "Close chat" }));
  expect(screen.getByRole("button", { name: "Summarize A" })).toHaveFocus();
  await user.click(screen.getByRole("button", { name: "Summarize A" }));
  await screen.findByText("Useful summary");
  expect(calls).toEqual([
    "GET /api/articles/a/chats?workspace_id=ws",
    "GET /api/articles/a/chats?workspace_id=ws",
  ]);
});

it("shows pending immediately, blocks duplicate paid requests and retries an uncertain request with the same operation ID", async () => {
  let reject!: (error: Error) => void;
  const operations: string[] = [];
  const transport: Transport = async <C extends ResponseContract>(
    path: string,
    init?: RequestInit,
  ) => {
    if (!init) return [] as unknown as ResponseValue<C>;
    operations.push(JSON.parse(String(init.body)).operationId);
    if (operations.length === 1)
      return (await new Promise<ArticleChat>((_, fail) => {
        reject = fail;
      })) as unknown as ResponseValue<C>;
    return saved() as unknown as ResponseValue<C>;
  };
  render(<Harness client={new AiClient(transport)} />);
  const button = screen.getByRole("button", { name: "Summarize A" });
  fireEvent.click(button);
  fireEvent.click(button);
  expect(screen.getByRole("status")).toHaveAttribute("aria-busy", "true");
  await waitFor(() => expect(operations).toHaveLength(1));
  reject(new Error("Connection interrupted; request acceptance is unknown"));
  await screen.findByText(
    /Connection interrupted; request acceptance is unknown/,
  );
  fireEvent.click(screen.getByRole("button", { name: "Retry", exact: true }));
  await screen.findByText("Useful summary");
  expect(operations).toHaveLength(2);
  expect(operations[1]).toBe(operations[0]);
});

it("gates generation without credentials but retains access to saved chats", async () => {
  const mutations: string[] = [];
  const transport: Transport = async <C extends ResponseContract>(
    path: string,
    init?: RequestInit,
  ) => {
    if (init) mutations.push(path);
    return (path.includes("/a/")
      ? []
      : [saved("b-chat", "b")]) as unknown as ResponseValue<C>;
  };
  const user = userEvent.setup();
  render(
    <Harness
      client={new AiClient(transport)}
      configured={{
        models: { summary: "deepseek-flash", verification: "deepseek-flash" },
        configured: false,
        enabled: false,
      }}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Summarize A" }));
  await screen.findByText(/Add and validate your DeepSeek API key/);
  expect(screen.queryByRole("button", { name: /profile/i })).toBeNull();
  await user.click(screen.getByRole("button", { name: "Summarize B" }));
  await screen.findByText("Useful summary");
  expect(
    screen.getByRole("button", { name: "Send", exact: true }),
  ).toBeDisabled();
  expect(mutations).toEqual([]);
});

it("keeps a separate draft per conversation across article switches and collapse/close", async () => {
  const transport: Transport = async <C extends ResponseContract>(
    path: string,
  ) =>
    [
      path.includes("/a/") ? saved() : saved("chat2", "b"),
    ] as unknown as ResponseValue<C>;
  const user = userEvent.setup();
  render(<Harness client={new AiClient(transport)} />);
  await user.click(screen.getByRole("button", { name: "Summarize A" }));
  await screen.findByText("Useful summary");
  await user.type(screen.getByLabelText("Message DeepSeek"), "Explain A");
  await user.click(screen.getByRole("button", { name: "Minimize chat" }));
  await user.click(screen.getByRole("button", { name: "Expand chat" }));
  expect(screen.getByLabelText("Message DeepSeek")).toHaveValue("Explain A");
  await user.click(screen.getByRole("button", { name: "Summarize B" }));
  await waitFor(() =>
    expect(screen.getByLabelText("Message DeepSeek")).toHaveValue(""),
  );
  await user.type(screen.getByLabelText("Message DeepSeek"), "Explain B");
  await user.click(screen.getByRole("button", { name: "Close chat" }));
  await user.click(screen.getByRole("button", { name: "Summarize A" }));
  await waitFor(() =>
    expect(screen.getByLabelText("Message DeepSeek")).toHaveValue("Explain A"),
  );
});

it("deduplicates sending and clears the submitted draft only after durable acceptance", async () => {
  let resolveSend!: (value: ArticleChat) => void;
  let sends = 0;
  const transport: Transport = async <C extends ResponseContract>(
    _path: string,
    init?: RequestInit,
  ) => {
    if (init) {
      sends += 1;
      return (await new Promise<ArticleChat>((resolve) => {
        resolveSend = resolve;
      })) as unknown as ResponseValue<C>;
    }
    return [saved()] as unknown as ResponseValue<C>;
  };
  const user = userEvent.setup();
  render(<Harness client={new AiClient(transport)} />);
  await user.click(screen.getByRole("button", { name: "Summarize A" }));
  await screen.findByText("Useful summary");
  await user.type(
    screen.getByLabelText("Message DeepSeek"),
    "What does this mean?",
  );
  const send = screen.getByRole("button", { name: "Send", exact: true });
  fireEvent.click(send);
  fireEvent.click(send);
  expect(send).toBeDisabled();
  expect(send).toHaveAttribute("aria-busy", "true");
  expect(sends).toBe(1);
  expect(screen.getByLabelText("Message DeepSeek")).toHaveValue(
    "What does this mean?",
  );
  resolveSend(saved());
  await waitFor(() =>
    expect(screen.getByLabelText("Message DeepSeek")).toHaveValue(""),
  );
});

it("preserves an uncertain accepted message operation across reconnect and article switches, blocking duplicate Send", async () => {
  const operations: { operationId: string; content: string }[] = [];
  const committed = {
    ...saved(),
    messages: [
      ...saved().messages,
      {
        id: "accepted-user",
        role: "user" as const,
        status: "complete" as const,
        content: "Explain the mechanism",
        createdAt: "2026-09-26T13:00:00Z",
      },
    ],
  };
  const transport: Transport = async <C extends ResponseContract>(
    path: string,
    init?: RequestInit,
  ) => {
    if (init) {
      operations.push(JSON.parse(String(init.body)));
      if (operations.length === 1)
        throw new Error("Response lost after the server committed");
      return committed as unknown as ResponseValue<C>;
    }
    if (path.includes("/b/"))
      return [saved("chat2", "b")] as unknown as ResponseValue<C>;
    if (path.includes("/articles/"))
      return [
        operations.length ? committed : saved(),
      ] as unknown as ResponseValue<C>;
    return committed as unknown as ResponseValue<C>;
  };
  const user = userEvent.setup();
  render(<Harness client={new AiClient(transport)} />);
  await user.click(screen.getByRole("button", { name: "Summarize A" }));
  await user.type(
    screen.getByLabelText("Message DeepSeek"),
    "Explain the mechanism",
  );
  await user.click(screen.getByRole("button", { name: "Send", exact: true }));
  await screen.findByText(/request outcome is unknown/);
  expect(
    screen.getByRole("button", { name: "Send", exact: true }),
  ).toBeDisabled();
  expect(screen.queryByRole("button", { name: "New summary" })).toBeNull();
  fireEvent.keyDown(screen.getByLabelText("Message DeepSeek"), {
    key: "Enter",
    ctrlKey: true,
  });
  // Reopening can reconcile reads but cannot silently repeat the uncertain POST.
  await user.click(screen.getByRole("button", { name: "Close chat" }));
  await user.click(screen.getByRole("button", { name: "Summarize A" }));
  await screen.findByRole("region", { name: "Your message" });
  expect(
    screen.getByRole("button", { name: "Send", exact: true }),
  ).toBeDisabled();
  await user.click(screen.getByRole("button", { name: "Close chat" }));
  await user.click(screen.getByRole("button", { name: "Summarize B" }));
  await user.click(screen.getByRole("button", { name: "Summarize A" }));
  await screen.findByText(/request outcome is unknown/);
  expect(
    screen.getByRole("button", { name: "Send", exact: true }),
  ).toBeDisabled();
  expect(screen.getByLabelText("Message DeepSeek")).toHaveValue(
    "Explain the mechanism",
  );
  expect(operations).toHaveLength(1);
  // A newer draft must survive acknowledgement of the original submitted text.
  await user.type(screen.getByLabelText("Message DeepSeek"), " in more detail");
  await user.click(screen.getByRole("button", { name: "Retry", exact: true }));
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Send", exact: true }),
    ).toBeEnabled(),
  );
  expect(operations).toHaveLength(2);
  expect(operations[1]).toEqual(operations[0]);
  expect(screen.getByLabelText("Message DeepSeek")).toHaveValue(
    "Explain the mechanism in more detail",
  );
});

it("allows correcting a definitively rejected message without losing its draft", async () => {
  const operations: { operationId: string; content: string }[] = [];
  const transport: Transport = async <C extends ResponseContract>(
    _path: string,
    init?: RequestInit,
  ) => {
    if (!init) return [saved()] as unknown as ResponseValue<C>;
    operations.push(JSON.parse(String(init.body)));
    if (operations.length === 1)
      throw new ApiError(422, "Message exceeds the configured input limit");
    return saved() as unknown as ResponseValue<C>;
  };
  const user = userEvent.setup();
  render(<Harness client={new AiClient(transport)} />);
  await user.click(screen.getByRole("button", { name: "Summarize A" }));
  await user.type(
    screen.getByLabelText("Message DeepSeek"),
    "Rejected question",
  );
  await user.click(screen.getByRole("button", { name: "Send", exact: true }));
  await screen.findByText("Message exceeds the configured input limit");
  expect(screen.getByLabelText("Message DeepSeek")).toHaveValue(
    "Rejected question",
  );
  await user.clear(screen.getByLabelText("Message DeepSeek"));
  await user.type(screen.getByLabelText("Message DeepSeek"), "Short question");
  await user.click(screen.getByRole("button", { name: "Send", exact: true }));
  await waitFor(() =>
    expect(screen.getByLabelText("Message DeepSeek")).toHaveValue(""),
  );
  expect(operations[1].operationId).not.toBe(operations[0].operationId);
  expect(operations[1].content).toBe("Short question");
});

it("never turns Reconnect into a paid initial summary after key setup", async () => {
  const mutations: string[] = [];
  const transport: Transport = async <C extends ResponseContract>(
    path: string,
    init?: RequestInit,
  ) => {
    if (init) {
      mutations.push(path);
      return saved() as unknown as ResponseValue<C>;
    }
    return [] as unknown as ResponseValue<C>;
  };
  const client = new AiClient(transport),
    user = userEvent.setup();
  const view = render(
    <Harness
      client={client}
      configured={{
        models: { summary: "deepseek-flash", verification: "deepseek-flash" },
        configured: false,
        enabled: false,
      }}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Summarize A" }));
  await screen.findByText(/Add and validate your DeepSeek API key/);
  view.rerender(<Harness client={client} />);
  await user.click(screen.getByRole("button", { name: "Reconnect" }));
  await screen.findByText(
    "No saved conversation yet. Click Summarize to start one.",
  );
  expect(mutations).toEqual([]);
  await user.click(screen.getByRole("button", { name: "Summarize A" }));
  await screen.findByText("Useful summary");
  expect(mutations).toEqual(["/api/articles/a/chat"]);
});

it("clears conversation state when account ownership changes", async () => {
  const transport: Transport = async <C extends ResponseContract>() =>
    [saved()] as unknown as ResponseValue<C>;
  const client = new AiClient(transport),
    user = userEvent.setup();
  const view = render(<Harness client={client} />);
  await user.click(screen.getByRole("button", { name: "Summarize A" }));
  await screen.findByText("Useful summary");
  await user.type(
    screen.getByLabelText("Message DeepSeek"),
    "Private question",
  );
  view.rerender(<Harness client={client} account="another-account" />);
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  await user.click(screen.getByRole("button", { name: "Summarize A" }));
  await screen.findByText("Useful summary");
  expect(screen.getByLabelText("Message DeepSeek")).toHaveValue("");
});

it("polls only an active visible conversation and reconnects without a paid request", async () => {
  vi.useFakeTimers();
  try {
    let reads = 0;
    const transport: Transport = async <C extends ResponseContract>(
      path: string,
      init?: RequestInit,
    ) => {
      expect(init).toBeUndefined();
      if (path.includes("/articles/"))
        return [
          { ...saved(), status: "generating" },
        ] as unknown as ResponseValue<C>;
      reads += 1;
      const chat = {
        ...saved(),
        status: reads > 1 ? "completed" : "generating",
      };
      return (path.includes("/changes")
        ? { revision: String(reads), chat }
        : chat) as unknown as ResponseValue<C>;
    };
    render(<Harness client={new AiClient(transport)} />);
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Summarize A" }));
    });
    expect(reads).toBe(0);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(reads).toBe(1);
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Close chat" }));
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(4000);
    });
    expect(reads).toBe(1);
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Summarize A" }));
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(reads).toBe(2);
    expect(screen.getByRole("status")).toHaveTextContent("");
    await act(async () => {
      await vi.advanceTimersByTimeAsync(4000);
    });
    expect(reads).toBe(2);
  } finally {
    vi.useRealTimers();
  }
});

it("opens latest saved summary without a version picker or regeneration action", async () => {
  const old = saved("old"),
    newest = {
      ...saved("new"),
      createdAt: "2026-09-27T12:00:00Z",
      messages: [{ ...saved().messages[0], content: "New summary text" }],
    };
  const mutations: unknown[] = [];
  const transport: Transport = async <C extends ResponseContract>(
    _path: string,
    init?: RequestInit,
  ) => {
    if (init) {
      mutations.push(JSON.parse(String(init.body)));
      return newest as unknown as ResponseValue<C>;
    }
    return [old, newest] as unknown as ResponseValue<C>;
  };
  const user = userEvent.setup();
  render(<Harness client={new AiClient(transport)} />);
  await user.click(screen.getByRole("button", { name: "Summarize A" }));
  await screen.findByText("New summary text");
  expect(screen.queryByRole("combobox")).toBeNull();
  expect(screen.queryByLabelText("Provider request costs")).toBeNull();
  expect(mutations).toHaveLength(0);
  expect(screen.queryByRole("button", { name: "New summary" })).toBeNull();
});

it.each(["failed", "interrupted", "cancelled"] as const)(
  "does not leave an empty terminal %s response spinning",
  async (status) => {
    const terminal: ArticleChat = {
      ...saved(),
      status,
      messages: [
        {
          ...saved().messages[0],
          status: status === "failed" ? "failed" : "interrupted",
          content: "",
        },
      ],
    };
    const transport: Transport = async <C extends ResponseContract>() =>
      [terminal] as unknown as ResponseValue<C>;
    render(<Harness client={new AiClient(transport)} />);
    fireEvent.click(screen.getByRole("button", { name: "Summarize A" }));
    await screen.findByText("No summary is available for this attempt.");
    expect(screen.queryByLabelText("Waiting for response")).toBeNull();
    expect(screen.getByRole("status")).toHaveAttribute("aria-busy", "false");
    expect(
      screen.queryByRole("button", { name: "Stop", exact: true }),
    ).toBeNull();
    expect(
      screen.getByRole("button", { name: "Retry verification", exact: true }),
    ).toBeEnabled();
  },
);
