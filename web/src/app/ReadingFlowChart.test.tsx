import { fireEvent, render, screen } from "@testing-library/preact";
import { ReadingFlowChart } from "./ReadingFlowChart";

describe("arrived and read chart", () => {
  const days = [
    { key: "2026-09-25", label: "Sep 25, 2026", arrived: 8, count: 4 },
    { key: "2026-09-29", label: "Sep 29, 2026", arrived: 0, count: 0 },
  ];
  it("renders independent series with one scale and an immediate hover/focus tooltip", () => {
    render(<ReadingFlowChart days={days} status="ready" />);
    const day = screen.getByRole("group", {
      name: "Sep 25, 2026: 8 arrived, 4 read",
    });
    expect((day.children[0] as HTMLElement).style.height).toBe("100%");
    expect((day.children[1].children[0] as HTMLElement).style.height).toBe(
      "50%",
    );
    fireEvent.mouseEnter(day);
    expect(screen.getByRole("tooltip")).toHaveTextContent(
      "Arrived: 8 · Read: 4",
    );
    fireEvent.mouseLeave(day.parentElement!);
    expect(screen.queryByRole("tooltip")).toBeNull();
    fireEvent.focus(
      screen.getByRole("group", { name: "Sep 29, 2026: 0 arrived, 0 read" }),
    );
    expect(screen.getByRole("tooltip")).toHaveTextContent(
      "Arrived: 0 · Read: 0",
    );
  });
  it("keeps the same chart scaffold while loading and supports all-zero days", () => {
    const { rerender } = render(
      <ReadingFlowChart
        days={days.map((d) => ({ ...d, count: 0, arrived: 0 }))}
        status="loading"
      />,
    );
    const section = screen.getByRole("region");
    expect(section).toHaveAttribute("aria-busy", "true");
    expect(screen.getAllByRole("group")).toHaveLength(2);
    expect(screen.getAllByRole("group")[0]).toHaveAttribute("tabindex", "-1");
    rerender(<ReadingFlowChart days={days} status="ready" />);
    expect(screen.getByRole("region")).toBe(section);
    expect(section).toHaveAttribute("aria-busy", "false");
    rerender(<ReadingFlowChart days={days} status="error" />);
    expect(section).toHaveAttribute("aria-busy", "false");
    expect(screen.getAllByRole("group")[0]).toHaveAttribute("tabindex", "-1");
  });
});

it("hides only the three requested dates while preserving slots and excluding their counts from scale", () => {
  const days = [25, 26, 27, 28, 29].map((d) => ({
    key: `2026-09-${d}`,
    label: `Sep ${d}, 2026`,
    arrived: d === 25 || d === 29 ? 8 : 10000,
    count: 4,
  }));
  const original = structuredClone(days);
  const { container } = render(
    <ReadingFlowChart days={days} status="ready" onReadDay={() => {}} />,
  );
  expect(container.querySelectorAll(".reading-flow__day")).toHaveLength(5);
  expect(screen.getAllByRole("group")).toHaveLength(2);
  for (const d of [26, 27, 28])
    expect(
      screen.queryByRole("button", {
        name: `Show articles marked read on Sep ${d}, 2026`,
      }),
    ).toBeNull();
  const visible = screen.getByRole("group", {
    name: "Sep 29, 2026: 8 arrived, 4 read",
  });
  expect((visible.children[0] as HTMLElement).style.height).toBe("100%");
  fireEvent.mouseEnter(visible);
  expect(screen.getByRole("tooltip")).toHaveTextContent("Arrived: 8 · Read: 4");
  const hidden = container.querySelectorAll(".reading-flow__day")[3];
  expect(hidden.children).toHaveLength(0);
  fireEvent.mouseEnter(hidden);
  expect(screen.queryByRole("tooltip")).toBeNull();
  expect(days).toEqual(original);
});
