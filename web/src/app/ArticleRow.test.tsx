import { fireEvent, render, screen } from "@testing-library/preact";
import { describe, expect, it, vi } from "vitest";
import { articles } from "../test/mockClient";
import { ArticleRow } from "./ArticleRow";

describe("untitled source posts", () => {
  it("labels the absence separately without replacing the authored title or caption", () => {
    const article = { ...articles[0], title: "", excerpt: "Original caption" };
    const onOpen = vi.fn();
    const props = {
      article,
      selected: false,
      laterPending: false,
      onOpen,
      onLater: vi.fn(),
    };
    const { rerender } = render(<ArticleRow {...props} />);
    const label = screen.getByLabelText("Post has no title");
    expect(label).toHaveTextContent("Untitled post");
    expect(screen.getByText("Original caption")).toBeVisible();
    fireEvent.click(label);
    expect(onOpen).toHaveBeenCalledOnce();
    rerender(
      <ArticleRow {...props} article={{ ...article, fullText: "pending" }} />,
    );
    expect(screen.getByLabelText("Post has no title")).toBe(label);
    expect(article.title).toBe("");
  });
});
