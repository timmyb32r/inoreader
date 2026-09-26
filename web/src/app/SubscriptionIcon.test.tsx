import { fireEvent, render, screen } from "@testing-library/preact";
import { describe, expect, it } from "vitest";
import { faviconUrl, SubscriptionIcon } from "./SubscriptionIcon";

describe("SubscriptionIcon", () => {
  it("loads the source-origin favicon without sending a reader referrer", () => {
    const { container } = render(<SubscriptionIcon name="Estuary" sourceUrl="https://estuary.dev/blog/rss.xml?format=rss" />);
    const image = container.querySelector("img") as HTMLImageElement;
    expect(image.src).toBe("https://estuary.dev/favicon.ico");
    expect(image).toHaveAttribute("referrerpolicy", "no-referrer");
    expect(image).toHaveAttribute("width", "27");
    expect(image).toHaveAttribute("height", "27");
    fireEvent.load(image);
    expect(image).toHaveClass("loaded");
  });

  it("keeps the fixed-size letter fallback when an icon fails", () => {
    const { container } = render(<SubscriptionIcon name="MariaDB" sourceUrl="https://mariadb.org/feed/" />);
    expect(screen.getByText("M")).toBeVisible();
    fireEvent.error(container.querySelector("img")!);
    expect(container.querySelector("img")).not.toBeInTheDocument();
    expect(screen.getByText("M")).toBeVisible();
    expect(container.querySelector(".subscription-icon")).toHaveStyle({ width: "27px", height: "27px" });
  });

  it("does not create image requests for invalid or unsupported source URLs", () => {
    expect(faviconUrl("javascript:alert(1)")).toBeUndefined();
    expect(faviconUrl("not a URL")).toBeUndefined();
  });
});
