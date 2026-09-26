import { fireEvent, render, screen } from "@testing-library/preact";
import { describe, expect, it } from "vitest";
import { SubscriptionIcon } from "./SubscriptionIcon";

describe("SubscriptionIcon", () => {
  it("renders the icon cached by the server without making a publisher request", () => {
    const dataUrl = "data:image/png;base64,aWNvbg==";
    const { container } = render(<SubscriptionIcon name="Estuary" iconDataUrl={dataUrl} />);
    const image = container.querySelector("img") as HTMLImageElement;
    expect(image.src).toBe(dataUrl);
    expect(image).toHaveAttribute("width", "27");
    expect(image).toHaveAttribute("height", "27");
    fireEvent.load(image);
    expect(image).toHaveClass("loaded");
  });

  it("keeps the fixed-size letter fallback when a cached icon fails", () => {
    const { container } = render(<SubscriptionIcon name="MariaDB" iconDataUrl="data:image/png;base64,broken" />);
    expect(screen.getByText("M")).toBeVisible();
    fireEvent.error(container.querySelector("img")!);
    expect(container.querySelector("img")).not.toBeInTheDocument();
    expect(screen.getByText("M")).toBeVisible();
    expect(container.querySelector(".subscription-icon")).toHaveStyle({ width: "27px", height: "27px" });
  });

  it("does not create an image request when the database has no icon", () => {
    const { container } = render(<SubscriptionIcon name="Unknown" />);
    expect(container.querySelector("img")).not.toBeInTheDocument();
  });
});
