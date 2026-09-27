import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/preact";
import { mockClient } from "../test/mockClient";
import { PublicationHistory } from "./PublicationHistory";

const history = {
  days: [
    { date: "2024-02-29", count: 2 },
    { date: "2025-01-01", count: 3 },
    { date: "2025-01-03", count: 1 },
  ],
  undated: 5,
  conflicting: 1,
};
it("switches between complete daily, monthly and yearly counts, including gaps and leap days", async () => {
  const client = mockClient();
  const request = vi
    .spyOn(client, "publicationHistory")
    .mockResolvedValue(history);
  render(<PublicationHistory client={client} subscriptionId="sub" />);
  expect(await screen.findByText(/6 dated articles/)).toBeVisible();
  const months = screen.getByRole("group", { name: "Articles by months" });
  expect(
    within(months).getByRole("button", { name: "Jan 2025 · 4 articles" }),
  ).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Days", exact: true }));
  expect(
    within(screen.getByRole("group", { name: "Articles by days" })).getByRole(
      "button",
      { name: "2025-01-02 · 0 articles" },
    ),
  ).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Years", exact: true }));
  fireEvent.click(screen.getByRole("button", { name: "2024 · 2 articles" }));
  expect(
    screen.getByRole("heading", { name: "2024 at a glance" }),
  ).toBeVisible();
  expect(
    within(screen.getByRole("region", { name: "Feb 2024" })).getByRole(
      "button",
      { name: "2024-02-29 · 2 articles" },
    ),
  ).toBeVisible();
  expect(
    screen.getByText(/5 without a publication date · 1 with conflicting dates/),
  ).toBeVisible();
  expect(request).toHaveBeenCalledTimes(1);
});

it("keeps the calendar structure during loading, reports errors and prevents repeated retry", async () => {
  const client = mockClient();
  let reject!: (error: Error) => void;
  const request = vi.spyOn(client, "publicationHistory").mockImplementation(
    () =>
      new Promise((_, no) => {
        reject = no;
      }),
  );
  const { container } = render(
    <PublicationHistory client={client} subscriptionId="sub" />,
  );
  expect(
    screen.getByRole("region", { name: "Publication history" }),
  ).toHaveAttribute("aria-busy", "true");
  expect(container.querySelectorAll(".publication-month")).toHaveLength(12);
  reject(new Error("Request failed"));
  expect(await screen.findByRole("alert")).toHaveTextContent("Request failed");
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  expect(
    screen.queryByRole("button", { name: "Retry" }),
  ).not.toBeInTheDocument();
  expect(container.querySelectorAll(".publication-month")).toHaveLength(12);
  await waitFor(() => expect(request).toHaveBeenCalledTimes(2));
});

it("shows an immediate tooltip and an integer scale for every interval", async () => {
  const client = mockClient();
  vi.spyOn(client, "publicationHistory").mockResolvedValue(history);
  const { container } = render(
    <PublicationHistory client={client} subscriptionId="sub" />,
  );
  await screen.findByText(/6 dated articles/);
  for (const scale of ["Months", "Days", "Years"]) {
    fireEvent.click(screen.getByRole("button", { name: scale, exact: true }));
    const bar = within(
      screen.getByRole("group", { name: `Articles by ${scale.toLowerCase()}` }),
    ).getAllByRole("button")[0];
    expect(bar).not.toHaveAttribute("title");
    fireEvent.mouseEnter(bar);
    expect(screen.getByRole("tooltip")).toHaveTextContent(
      bar.getAttribute("aria-label")!,
    );
    const ticks = [...container.querySelectorAll(".publication-axis>span")].map(
      (node) => Number(node.textContent),
    );
    expect(ticks[0]).toBe(0);
    expect(ticks.every(Number.isInteger)).toBe(true);
    expect(ticks.at(-1)).toBeGreaterThanOrEqual(scale === "Days" ? 3 : 4);
    fireEvent.mouseLeave(container.querySelector(".publication-chart-frame")!);
    expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
    fireEvent.focus(bar);
    expect(screen.getByRole("tooltip")).toBeVisible();
    fireEvent.keyDown(bar, { key: "Escape" });
    expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
  }
});
