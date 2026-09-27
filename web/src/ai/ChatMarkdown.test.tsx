import { fireEvent, render, screen } from "@testing-library/preact";
import { ChatMarkdown } from "./ChatMarkdown";

it("renders supported Markdown and copies source quote text without formatting markers", async () => {
  const copy = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText: copy },
  });
  render(
    <ChatMarkdown
      text={
        "# Original title\n\n**Kafka** uses `partitions`.\n\n- A detail\n- A caveat\n\n> Exact statement.\n> Second line.\n\n[Source](https://example.test/article)"
      }
    />,
  );
  expect(screen.getByText("Kafka").tagName).toBe("STRONG");
  expect(screen.getByText("partitions").tagName).toBe("CODE");
  expect(screen.getAllByRole("listitem")).toHaveLength(2);
  expect(screen.getByRole("link", { name: "Source" })).toHaveAttribute(
    "rel",
    "noopener noreferrer",
  );
  fireEvent.click(screen.getByRole("button", { name: "Copy quote" }));
  expect(copy).toHaveBeenCalledWith("Exact statement.\nSecond line.");
});

it("never executes model HTML, scripts, unsafe URLs or remote images", () => {
  const { container } = render(
    <ChatMarkdown
      text={
        '<script>alert(1)</script>\n\n<img src=x onerror=alert(1)>\n\n[Unsafe](javascript:alert) [Unsafe data](data:text/html,evil)\n\n![Leak](https://remote.test/tracking)\n\n```html\n<iframe src="https://example.test"/>\n```'
      }
    />,
  );
  expect(container.querySelector("script,img,iframe,a")).toBeNull();
  expect(screen.getByText("<script>alert(1)</script>")).toBeVisible();
  expect(
    screen.getByText('<iframe src="https://example.test"/>'),
  ).toBeVisible();
});
