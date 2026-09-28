import { render, screen, fireEvent, waitFor } from "@testing-library/preact";
import { describe, it, expect, vi } from "vitest";
import { WikiEditor } from "./WikiEditor";
import type { WikiClient } from "../api/wiki";
const limits = {
  name_bytes: 512,
  markdown_bytes: 10000,
  search_bytes: 512,
  search_excerpt_characters: 200,
  page_size: 50,
  draft_save_delay_ms: 10000,
};
describe("wiki editor", () => {
  it("shows immediate pending feedback and prevents double publication", async () => {
    let finish!: (v: any) => void;
    const client = {
      draft: vi.fn().mockResolvedValue({ draft: null }),
      write: vi.fn(
        () =>
          new Promise((r) => {
            finish = r;
          }),
      ),
      discard: vi.fn().mockResolvedValue(undefined),
    } as unknown as WikiClient;
    const saved = vi.fn();
    render(
      <WikiEditor
        client={client}
        namespace="n"
        id="p"
        page={null}
        initialName="Page"
        limits={limits}
        onSaved={saved}
        onCancel={() => {}}
        onDirty={() => {}}
      />,
    );
    const button = screen.getByRole("button", { name: "Save" });
    await waitFor(() => expect(button).not.toBeDisabled());
    fireEvent.click(button);
    fireEvent.click(button);
    await waitFor(() => expect(client.write).toHaveBeenCalledTimes(1));
    expect(button).toHaveAttribute("aria-busy", "true");
    expect(button).toBeDisabled();
    finish({ id: "p", name: "Page", markdown: "", revision: "r" });
    await waitFor(() => expect(saved).toHaveBeenCalledTimes(1));
  });
  it("restores an exact private draft", async () => {
    const client = {
      draft: vi.fn().mockResolvedValue({
        draft: {
          id: "p",
          page: null,
          base_revision: null,
          name: " Exact ",
          markdown: "中文\n\nOriginal",
        },
      }),
    } as unknown as WikiClient;
    render(
      <WikiEditor
        client={client}
        namespace="n"
        id="p"
        page={null}
        initialName=""
        limits={limits}
        onSaved={() => {}}
        onCancel={() => {}}
        onDirty={() => {}}
      />,
    );
    await waitFor(() =>
      expect(screen.getByRole("textbox", { name: "Page name" })).toHaveValue(
        " Exact ",
      ),
    );
    expect(screen.getByRole("textbox", { name: "Markdown" })).toHaveValue(
      "中文\n\nOriginal",
    );
  });
});
