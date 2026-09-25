import { render, screen, waitFor } from "@testing-library/preact";
import userEvent from "@testing-library/user-event";
import { ApiClient, type Transport } from "../api/client";
import { RulesDialog } from "./RulesDialog";

type SetupOptions = { finalStatus?: "completed" | "cancelled"; holdStatus?: boolean };
function setup(options: SetupOptions = {}) {
  const calls: { path: string; body?: Record<string, unknown> }[] = [];
  let resolveStatus: ((value: unknown) => void) | undefined;
  const statusPromise = new Promise(resolve => { resolveStatus = resolve; });
  const transport: Transport = async <T,>(path: string, init?: RequestInit) => {
    const body = init?.body ? JSON.parse(String(init.body)) as Record<string, unknown> : undefined; calls.push({ path, body });
    if (path.startsWith("/api/rules?") && !init?.method) return [] as T;
    if (path.startsWith("/api/rules/preview")) return { matchedArticles: 7, sharedArticles: 3, totalSubscriptionArticles: 12, sampleArticleIds: ["article-a", "article-b"] } as T;
    if (path.startsWith("/api/rules/") && path.includes("/apply")) return { operationId: "operation", status: "queued" } as T;
    if (path.startsWith("/api/rule-applications/")) {
      if (options.holdStatus) return await statusPromise as T;
      return { operationId: "operation", status: options.finalStatus ?? "completed", evaluated: 12, cancelReason: options.finalStatus === "cancelled" ? "rule version changed" : undefined } as T;
    }
    if (path.startsWith("/api/rules") && init?.method === "POST") return { ...body, id: "rule" } as T;
    return undefined as T;
  };
  const client = new ApiClient(transport);
  const view = render(<RulesDialog client={client} workspaceId="workspace-a" subscriptionId="subscription" onClose={() => undefined}/>);
  return { calls, client, view, resolveStatus: (value: unknown) => resolveStatus?.(value) };
}

async function previewRule(user: ReturnType<typeof userEvent.setup>) {
  await user.type(screen.getByLabelText("New literal phrase"), "career");
  await user.selectOptions(screen.getByLabelText("Match field"), "title");
  await user.selectOptions(screen.getByLabelText("Action"), "move_to_trash");
  await user.click(screen.getByRole("button", { name: "Preview matches" }));
  await screen.findByText("7");
}

describe("RulesDialog", () => {
  it("shows matched, shared, total and sample scope from server preview", async () => {
    const { calls } = setup(); const user = userEvent.setup(); await previewRule(user);
    expect(screen.getByText("3")).toBeVisible(); expect(screen.getByText("12")).toBeVisible();
    expect(screen.getByText(/shared with linked subscriptions/)).toBeVisible();
    await user.click(screen.getByText("Sample article IDs (2)")); expect(screen.getByText("article-a")).toBeVisible();
    expect(calls.find(call => call.path.startsWith("/api/rules/preview"))?.body).toMatchObject({ subscriptionId: "subscription", field: "title", phrase: "career", action: "move_to_trash", enabled: true });
    expect(screen.getByRole("button", { name: "Save & apply existing" })).toBeEnabled();
    await user.type(screen.getByLabelText("New literal phrase"), "s");
    expect(screen.getByText(/rule changed/i)).toBeVisible(); expect(screen.getByRole("button", { name: "Save & apply existing" })).toBeDisabled();
  });

  it("reports terminal bulk progress and cancellation reason", async () => {
    setup({ finalStatus: "cancelled" }); const user = userEvent.setup(); await previewRule(user);
    await user.dblClick(screen.getByRole("button", { name: "Save & apply existing" }));
    expect(await screen.findByText("Bulk apply: cancelled")).toBeVisible(); expect(screen.getByText("12 articles evaluated")).toBeVisible(); expect(screen.getByText("rule version changed")).toBeVisible();
  });

  it("ignores old polling responses after a workspace context switch", async () => {
    const { client, view, resolveStatus, calls } = setup({ holdStatus: true }); const user = userEvent.setup(); await previewRule(user);
    await user.click(screen.getByRole("button", { name: "Save & apply existing" })); await screen.findByText("Bulk apply: queued");
    view.rerender(<RulesDialog client={client} workspaceId="workspace-b" subscriptionId="subscription-b" onClose={() => undefined}/>);
    resolveStatus({ operationId: "operation", status: "failed", evaluated: 4, cancelReason: "stale operation" });
    await waitFor(() => expect(calls.some(call => call.path.includes("workspace_id=workspace-b"))).toBe(true));
    expect(screen.queryByText("stale operation")).not.toBeInTheDocument(); expect(screen.getByText("Progress and cancellation reasons will appear here.")).toBeVisible();
  });
});
