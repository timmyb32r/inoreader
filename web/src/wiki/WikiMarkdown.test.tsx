import { render, screen, fireEvent, waitFor } from "@testing-library/preact";
import { describe, it, expect, vi } from "vitest";
import { WikiMarkdown } from "./WikiMarkdown";
describe("safe wiki Markdown", () => {
  it("renders text, code, tables and scoped links without executing HTML or embedding images", async () => {
    const onLink = vi.fn();
    const { container } = render(
      <WikiMarkdown
        onLink={onLink}
        text={
          "# Title\n\n[[ 页面 ]]\n\n<script>alert(1)</script> ![spy](https://example.com/pixel) [bad](javascript:alert)\n\n| Name | Value |\n| --- | --- |\n| **Bold** | 中文 |\n\n```\n[[not a link]]\n```"
        }
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "页面" }));
    await waitFor(() => expect(onLink).toHaveBeenCalledWith(" 页面 "));
    expect(container.querySelector("script,img,iframe")).toBeNull();
    expect(container.querySelector("table")).not.toBeNull();
    expect(container.querySelector("strong")?.textContent).toBe("Bold");
    expect(screen.queryByRole("link", { name: "bad" })).toBeNull();
    expect(screen.queryByRole("button", { name: "not a link" })).toBeNull();
  });
  it("ordinary Markdown cannot link across wiki namespaces", () => {
    render(
      <WikiMarkdown
        onLink={() => {}}
        text={`[foreign](${location.origin}/wiki/other/page/id) [safe](https://example.com/article)`}
      />,
    );
    expect(screen.queryByRole("link", { name: "foreign" })).toBeNull();
    expect(screen.getByRole("link", { name: "safe" })).toHaveAttribute(
      "rel",
      "noopener noreferrer",
    );
  });
});

it("links bare public URLs without linking code or internal namespaces", () => {
  render(
    <WikiMarkdown
      onLink={() => {}}
      text={`https://github.com/activepieces/activepieces

\`https://example.com/code\`

${location.origin}/wiki/foreign/page/id`}
    />,
  );
  expect(
    screen.getByRole("link", {
      name: "https://github.com/activepieces/activepieces",
    }),
  ).toHaveAttribute("href", "https://github.com/activepieces/activepieces");
  expect(
    screen.queryByRole("link", { name: "https://example.com/code" }),
  ).toBeNull();
  expect(
    screen.queryByRole("link", {
      name: `${location.origin}/wiki/foreign/page/id`,
    }),
  ).toBeNull();
});
