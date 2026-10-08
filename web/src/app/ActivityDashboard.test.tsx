import { act, render, screen, waitFor } from "@testing-library/preact";
import { ApiClient } from "../api/client";
import { ActivityDashboard } from "./ActivityDashboard";

describe("activity dashboard", () => {
  it("shows server article counts, never the former local elapsed time", async () => {
    const now = new Date();
    const day = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
    Object.defineProperty(window, "localStorage", {
      configurable: true,
      value: { getItem: () => JSON.stringify({ [day]: 3600000 }) },
    });
    const transport = vi
      .fn()
      .mockResolvedValue({ days: [{ day, count: 7, arrived: 12 }] });
    render(
      <ActivityDashboard
        client={new ApiClient(transport)}
        workspaceId="one"
        workspaceName="Personal"
        onOpenLibrary={() => undefined}
      />,
    );
    await waitFor(() =>
      expect(
        screen.getByText("articles marked read today").previousElementSibling,
      ).toHaveTextContent("7"),
    );
    const grid = screen.getByRole("grid", {
      name: "Daily articles marked read",
    });
    expect(grid.querySelectorAll('[role="gridcell"]')).toHaveLength(364);
    const firstDay = new Date(now);
    firstDay.setDate(firstDay.getDate() - 363);
    expect(
      (grid.querySelector('[role="gridcell"]') as HTMLElement).style
        .gridRowStart,
    ).toBe(String(((firstDay.getDay() + 6) % 7) + 1));
    expect(
      screen.getByRole("gridcell", { name: /: 7 articles$/ }),
    ).toBeVisible();
    expect(transport.mock.calls[0][0]).toContain(
      "/api/workspaces/one/reading-activity?timezone=",
    );
    expect(screen.queryByText(/minutes/)).toBeNull();
  });

  it("keeps the calendar mounted during loading and failure; deduplicates refresh", async () => {
    let reject!: (error: Error) => void;
    const transport = vi.fn().mockImplementation(
      () =>
        new Promise((_, no) => {
          reject = no;
        }),
    );
    render(
      <ActivityDashboard
        client={new ApiClient(transport)}
        workspaceId="one"
        workspaceName="Personal"
        onOpenLibrary={() => undefined}
      />,
    );
    const grid = screen.getByRole("grid");
    const button = screen.getByRole("button", { name: "Начать чтение" });
    expect(grid).toHaveAttribute("aria-busy", "true");
    act(() => {
      window.dispatchEvent(new Event("focus"));
    });
    expect(transport).toHaveBeenCalledTimes(1);
    await act(async () => reject(new Error("offline")));
    expect(
      screen
        .getAllByRole("status")
        .every((node) => node.textContent?.includes("Could not load")),
    ).toBe(true);
    expect(screen.getByRole("grid")).toBe(grid);
    expect(grid.querySelectorAll('[role="gridcell"]')).toHaveLength(364);
    expect(screen.getByRole("button", { name: "Начать чтение" })).toBe(button);
    expect(
      screen.getByText("articles marked read today").previousElementSibling,
    ).toHaveTextContent("—");
  });
});
