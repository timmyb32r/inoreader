import { render, screen, fireEvent, waitFor } from "@testing-library/preact";
import { describe, it, expect, vi } from "vitest";
import type { WikiClient } from "../api/wiki";
import { CreateSubscriptionPage } from "./CreateSubscriptionPage";
import { ArticleWikiLink } from "../app/ArticleWikiLink";

describe("subscription wiki creation", () => {
  it("shows creation only for truly unlinked subscriptions", async () => {
    const client = {
      binding: vi
        .fn()
        .mockResolvedValue({ linked: true, namespace: null, page: null }),
    } as unknown as WikiClient;
    const { rerender } = render(
      <ArticleWikiLink
        client={client}
        subscription="s"
        name="Source"
        onNavigate={() => {}}
      />,
    );
    await waitFor(() => expect(client.binding).toHaveBeenCalled());
    expect(
      screen.queryByRole("button", { name: "Create and link wiki page" }),
    ).toBeNull();
    vi.mocked(client.binding).mockResolvedValue({
      linked: false,
      namespace: null,
      page: null,
    });
    rerender(
      <ArticleWikiLink
        client={client}
        subscription="other"
        name="Source"
        onNavigate={() => {}}
      />,
    );
    expect(
      await screen.findByRole("button", { name: "Create and link wiki page" }),
    ).toBeTruthy();
  });
  it("locks immediately, keeps controls mounted, and retries binding without another page", async () => {
    let finish!: (v: unknown) => void;
    const client = {
      limits: vi.fn().mockResolvedValue({ page_size: 50 }),
      namespaces: vi.fn().mockResolvedValue({
        items: [
          { id: "n", name: "Private", role: "owner" },
          { id: "r", name: "Read only", role: "reader" },
        ],
        has_more: false,
      }),
      subscriptionRoot: vi.fn().mockResolvedValue({ id: "root" }),
      write: vi
        .fn()
        .mockImplementationOnce(
          () =>
            new Promise<unknown>((r) => {
              finish = r;
            }),
        )
        .mockResolvedValue({ id: "page", revision: "parented" }),
      bind: vi
        .fn()
        .mockRejectedValueOnce(new Error("Binding failed"))
        .mockResolvedValue(undefined),
    } as unknown as WikiClient;
    const created = vi.fn();
    render(
      <CreateSubscriptionPage
        client={client}
        subscription="s"
        name="Source"
        onClose={() => {}}
        onCreated={created}
      />,
    );
    const button = screen.getByRole("button", { name: "Create and link" });
    await waitFor(() => expect(button).not.toBeDisabled());
    expect(screen.queryByRole("option", { name: "Read only" })).toBeNull();
    const field = screen.getByRole("textbox", { name: "Wiki page name" });
    fireEvent.click(button);
    fireEvent.click(button);
    expect(button).toHaveAttribute("aria-busy", "true");
    await waitFor(() => expect(client.write).toHaveBeenCalledTimes(1));
    expect(screen.getByRole("textbox", { name: "Wiki page name" })).toBe(field);
    finish({ id: "page", revision: "created" });
    await screen.findByText("Binding failed");
    await waitFor(() => expect(button).not.toBeDisabled());
    fireEvent.click(button);
    await waitFor(() => expect(created).toHaveBeenCalledTimes(1));
    expect(client.write).toHaveBeenCalledTimes(2);
    expect(client.bind).toHaveBeenCalledTimes(2);
    expect(vi.mocked(client.write).mock.calls[0][1].change).toEqual({
      action: "save",
      name: "Source",
      markdown: "",
    });
  });
});
