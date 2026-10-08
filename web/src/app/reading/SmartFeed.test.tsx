import { fireEvent, render, screen, waitFor } from "@testing-library/preact";
import type { ApiClient } from "../../api/client";
import type { SmartFeed as Feed } from "../../api/generated";
import { SmartFeed } from "./SmartFeed";
const data: Feed = {
  profile: {
    prompt: "Research, not marketing",
    revision: "1",
    trainingCount: 139,
  },
  total: 2,
  scored: 1,
  failed: 0,
  nextCursor: null,
  articles: [
    {
      id: "research",
      title: "Database research",
      excerpt: "An experiment",
      prediction: {
        score: 9,
        reason: "Глубокая инженерия",
        confidence: "high",
      },
      error: null,
    },
    {
      id: "pending",
      title: "Unknown article",
      excerpt: "",
      prediction: null,
      error: null,
    },
  ],
};
describe("personal smart feed", () => {
  it("shows prediction evidence and does not silently reorder or mark read", async () => {
    const feed = vi.fn().mockResolvedValue(data);
    const navigate = vi.fn();
    const client = { interests: { feed } } as unknown as ApiClient;
    render(
      <SmartFeed
        client={client}
        workspace="workspace"
        navigate={navigate}
        onExit={() => {}}
      />,
    );
    await screen.findByText("Глубокая инженерия");
    expect(screen.getByText("9/10")).toBeInTheDocument();
    expect(screen.getByText("Ещё не оценено")).toBeInTheDocument();
    expect(feed).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("link", { name: "Database research" }));
    expect(navigate).toHaveBeenCalledWith(
      "/reading?workspace=workspace&article=research&from=smart",
    );
  });
  it("locks profile saving immediately and preserves the draft on failure", async () => {
    let fail!: (value: Error) => void;
    const save = vi.fn().mockImplementation(
      () =>
        new Promise((_, reject) => {
          fail = reject;
        }),
    );
    const client = {
      interests: { feed: vi.fn().mockResolvedValue(data), save },
    } as unknown as ApiClient;
    render(
      <SmartFeed
        client={client}
        workspace="workspace"
        navigate={() => {}}
        onExit={() => {}}
      />,
    );
    await screen.findByText("Глубокая инженерия");
    fireEvent.click(screen.getByRole("button", { name: "Профиль интересов" }));
    const field = screen.getByRole("textbox", { name: "Профиль интересов" });
    fireEvent.input(field, { target: { value: "Edited interests" } });
    const button = screen.getByRole("button", { name: "Сохранить профиль" });
    fireEvent.click(button);
    fireEvent.click(button);
    expect(save).toHaveBeenCalledTimes(1);
    expect(button).toBeDisabled();
    expect(button).toHaveAttribute("aria-busy", "true");
    fail(new Error("Save failed"));
    await waitFor(() =>
      expect(screen.getByRole("status")).toHaveTextContent("Save failed"),
    );
    expect(field).toHaveValue("Edited interests");
    expect(
      screen.getByRole("button", { name: "Сохранить профиль" }),
    ).not.toBeDisabled();
  });
});
