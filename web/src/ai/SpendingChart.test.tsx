import { render, screen } from "@testing-library/preact";
import { SpendingChart } from "./SpendingChart";
import type { AiProfile } from "../api/ai";
it("shows separate exact costs and uncertain reserves for all modes", () => {
  const spending: NonNullable<AiProfile["spending"]> = {
    dailyLimitUsd: "3",
    today: "2026-09-29",
    resetsAt: "2026-09-29T21:00:00Z",
    spentUsd: "0.123456",
    reservedUsd: "0.5",
    remainingUsd: "2.376544",
    days: [
      {
        day: "2026-09-29",
        mode: "summary",
        spentUsd: "0.123456",
        reservedUsd: "0.5",
      },
    ],
  };
  render(<SpendingChart spending={spending} />);
  expect(screen.getAllByRole("img")).toHaveLength(30);
  expect(screen.getByRole("status")).toHaveTextContent("0.123456");
  expect(screen.getAllByRole("img").at(-1)).toHaveAccessibleName(
    /Summary \$0.123456, reserved \$0.5; Fact-check \$0/,
  );
  expect(screen.getByText("Terms")).toBeVisible();
});
