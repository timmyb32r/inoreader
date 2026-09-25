import { fireEvent, render, screen, within } from "@testing-library/preact";
import userEvent from "@testing-library/user-event";
import { App } from "./App";
import { mockClient } from "../test/mockClient";

const renderApp = () => render(<App client={mockClient()}/>);

describe("reader application", () => {
  it("opens an article and marks it read without changing saved state", async () => {
    const user = userEvent.setup(); renderApp();
    const row = (await screen.findByRole("heading", { name: "Async Rust without the hidden machinery", level: 2 })).closest("article")!;
    expect(row).not.toHaveClass("read");
    expect(within(row).getByRole("button", { name: "Remove from saved" })).toBeInTheDocument();
    await user.click(within(row).getByRole("button", { name: /Async Rust without/ }));
    expect(row).toHaveClass("read");
    expect(within(row).getByRole("button", { name: "Remove from saved" })).toBeInTheDocument();
  });

  it("keeps saved and later independent", async () => {
    const user = userEvent.setup(); renderApp();
    await screen.findByRole("heading",{name:"Async Rust without the hidden machinery",level:1});
    const reader=screen.getByRole("article",{name:"Article reader"});
    expect(within(reader).getByRole("button", { name: "Unsave" })).toHaveAttribute("aria-pressed", "true");
    expect(within(reader).getByRole("button", { name: "Read later" })).toHaveAttribute("aria-pressed", "false");
    await user.click(within(reader).getByRole("button", { name: "Read later" }));
    expect(within(reader).getByRole("button", { name: "Unsave" })).toHaveAttribute("aria-pressed", "true");
    expect(within(reader).getByRole("button", { name: "Remove from later" })).toHaveAttribute("aria-pressed", "true");
  });

  it("shows a disabled search stub and never renders an input", async () => {
    renderApp();
    const search = await screen.findAllByRole("button", { name: /Search/ });
    expect(search).toHaveLength(2);search.forEach(item=>expect(item).toBeDisabled());
    expect(screen.getByText("Coming later")).toBeVisible();
    expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();
  });

  it("requires a reason and retains it while validating pause", async () => {
    const user = userEvent.setup(); renderApp();
    const subscriptions=await screen.findByRole("navigation",{name:"Subscriptions"});
    await user.click(within(subscriptions).getByRole("button", { name: /This Week in Rust/ }));
    await user.click(screen.getByRole("button", { name: "Settings & shortcuts" }));
    await user.click(screen.getByRole("button", { name: "Pause This Week in Rust" }));
    const reason = screen.getByLabelText(/Reason/);
    const submit = screen.getByRole("button", { name: "Pause subscription" });
    fireEvent.blur(reason);
    expect(screen.getByText("Enter a reason before continuing.")).toBeVisible();
    await user.type(reason, "Reading backlog first");
    expect(reason).toHaveValue("Reading backlog first");
    expect(submit).toBeEnabled();
  });

  it("archives with a reason and restores without losing the library", async () => {
    const user = userEvent.setup(); renderApp();
    await user.click(await screen.findByRole("button", { name: /Data engineering/ }));
    await user.click(screen.getByRole("button", { name: "Archive workspace" }));
    await user.type(screen.getByLabelText(/Reason/), "Taking a break");
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Archive workspace" }));
    await new Promise((resolve) => setTimeout(resolve, 550));
    expect(screen.getByText(/workspace is archived/i)).toBeVisible();
    expect(screen.getByRole("heading",{name:"Async Rust without the hidden machinery",level:2})).toBeVisible();
  });

  it("supports keyboard next, previous, save and later", async () => {
    renderApp();
    await screen.findByRole("heading", { name: "Async Rust without the hidden machinery", level: 1 });
    fireEvent.keyDown(window,{key:"j"});
    expect(await screen.findByRole("heading", { name: "The durable queue is the product", level: 1 })).toBeVisible();
    fireEvent.keyDown(window,{key:"s"});
    expect(await screen.findByRole("button", { name: "Unsave" })).toBeVisible();
    fireEvent.keyDown(window,{key:"l"});
    expect(await screen.findByRole("button", { name: "Remove from later" })).toBeVisible();
    fireEvent.keyDown(window,{key:"k"});
    expect(screen.getByRole("heading", { name: "Async Rust without the hidden machinery", level: 1 })).toBeVisible();
  });

  it("preserves source languages", async () => {
    renderApp();
    expect(await screen.findByText("流式系统中的背压设计")).toBeVisible();
    await userEvent.setup().click(screen.getByRole("button",{name:/Data engineering/}));
    expect(screen.getByRole("button", { name: /Финансы/ })).toBeInTheDocument();
  });
});
