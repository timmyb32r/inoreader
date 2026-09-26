import { fireEvent, render, screen, within } from "@testing-library/preact";
import userEvent from "@testing-library/user-event";
import { App } from "./App";
import { articles, mockClient } from "../test/mockClient";
import { ApiClient, ApiError } from "../api/client";

const renderApp = () => render(<App client={mockClient()}/>);

describe("reader application", () => {
  it("opens a failed subscription's update log from the sidebar", async () => {
    history.replaceState({}, "", "/");
    const client = mockClient();
    const failed = { id:"failed",name:"Broken feed",sourceType:"feed" as const,count:0,status:"active" as const,error:"outbound request timed out" };
    vi.spyOn(client, "bootstrap").mockResolvedValue({
      account: { displayName: "Test", initials: "TB" },
      workspaces: [{ id: "ws", name: "Data engineering", archived: false }],
      activeWorkspaceId: "ws", subscriptions: [failed], articles: [], newArticleCount: 0,
    });
    vi.spyOn(client, "getSubscription").mockResolvedValue(failed);
    const activity = vi.spyOn(client, "subscriptionActivity").mockResolvedValue([{
      id:"attempt",occurredAt:"2026-09-26T10:00:00Z",successful:false,diagnostic:"connection refused",
    }]);
    const user = userEvent.setup();
    render(<App client={client}/>);

    await user.click(await screen.findByRole("button", { name: "Open update log for Broken feed" }));
    expect(window.location.pathname).toBe("/subscriptions/failed/activity");
    expect(await screen.findByText("connection refused")).toBeVisible();
    expect(activity).toHaveBeenCalledTimes(1);
  });

  it("uses the brand as a link back to the main library", async () => {
    history.replaceState({}, "", "/subscriptions");
    const user = userEvent.setup();
    renderApp();

    await user.click(await screen.findByRole("link", { name: "Reader home" }));
    expect(window.location.pathname).toBe("/");
    expect(await screen.findByRole("heading", { name: "All articles" })).toBeVisible();
  });

  it("centers an accessible spinner while bootstrap is pending", () => {
    const client = new ApiClient(async () => new Promise(() => undefined));
    render(<App client={client}/>);

    const loading = screen.getByRole("status", { name: "Opening your library" });
    expect(loading).toHaveClass("bootstrap-loading");
    expect(loading.querySelector(".bootstrap-loading__spinner")).not.toBeNull();
    expect(screen.queryByText("Opening your library…")).not.toBeInTheDocument();
  });

  it("replaces the bootstrap spinner with a retryable error", async () => {
    const client = new ApiClient(async () => {
      throw new ApiError(500, "storage operation failed");
    });
    render(<App client={client}/>);

    expect(await screen.findByRole("heading", { name: "Reader is unavailable" })).toBeVisible();
    expect(screen.getByText("storage operation failed")).toBeVisible();
    expect(screen.getByRole("button", { name: "Try again" })).toBeVisible();
    expect(screen.queryByText("Opening your library…")).not.toBeInTheDocument();
  });

  it("replaces pending full text in place when background extraction completes", async () => {
    const client = mockClient();
    const pending = { ...articles[0], fullText: "pending" as const, body: [] };
    vi.spyOn(client, "bootstrap").mockResolvedValue({
      account: { displayName: "Test", initials: "TB" },
      workspaces: [{ id: "ws", name: "Data engineering", archived: false }],
      activeWorkspaceId: "ws",
      subscriptions: [],
      articles: [pending],
      newArticleCount: 0,
    });
    const getArticle = vi.spyOn(client, "getArticle").mockResolvedValue(
      { ...pending, fullText: "ready", body: ["Extracted body"] },
    );

    render(<App client={client}/>);
    const readerStatus = await screen.findByText(/Fetching the full article/);
    expect(readerStatus).toBeVisible();
    expect(readerStatus.querySelector(".spinner")).not.toBeNull();
    const contentLoading = screen.getByRole("status", { name: "Loading full article content" });
    expect(contentLoading.querySelector(".spinner")).not.toBeNull();
    const row = screen.getByRole("heading", { name: pending.title, level: 2 }).closest("article");
    expect(row?.querySelector(".fulltext--pending .spinner")).not.toBeNull();

    expect(await screen.findByText("Extracted body", {}, { timeout: 2000 })).toBeVisible();
    expect(getArticle).toHaveBeenCalledTimes(1);
    expect(screen.queryByText(/Fetching the full article/)).not.toBeInTheDocument();
    expect(screen.queryByRole("status", { name: "Loading full article content" })).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: pending.title, level: 2 }).closest("article")).toBe(row);
  });

  it("renders sanitized article markup as structured content", async () => {
    const client = mockClient();
    const formatted = {
      ...articles[0],
      body: [],
      bodyHtml: '<h2>Benefits</h2><p>Use <strong>structured data</strong>.</p><ul><li>Fast</li></ul><img src="https://example.com/diagram.png" alt="CSV diagram">',
    };
    vi.spyOn(client, "bootstrap").mockResolvedValue({
      account: { displayName: "Test", initials: "TB" },
      workspaces: [{ id: "ws", name: "Data engineering", archived: false }],
      activeWorkspaceId: "ws", subscriptions: [], articles: [formatted], newArticleCount: 0,
    });
    render(<App client={client}/>);

    const reader = await screen.findByRole("article", { name: "Article reader" });
    expect(within(reader).getByRole("heading", { name: "Benefits", level: 2 })).toBeVisible();
    expect(within(reader).getByText("structured data").tagName).toBe("STRONG");
    expect(within(reader).getByText("Fast").closest("li")).not.toBeNull();
    expect(within(reader).getByRole("img", { name: "CSV diagram" })).toHaveAttribute("src", "https://example.com/diagram.png");
  });

  it("opens an account menu before an explicit sign out", async () => {
    const user = userEvent.setup();
    const client = mockClient();
    const signOut = vi.fn(async () => undefined);
    client.signOut = signOut;
    render(<App client={client}/>);

    const accountMenu = await screen.findByRole("button", { name: "Account menu" });
    await user.click(accountMenu);
    expect(signOut).not.toHaveBeenCalled();
    expect(screen.getByRole("menu")).toBeVisible();
    expect(screen.getByText("Test")).toBeVisible();

    await user.click(screen.getByRole("menuitem", { name: "Sign out" }));
    expect(signOut).toHaveBeenCalledTimes(1);
  });

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

  it("guards internal and browser navigation while a personal note is dirty", async () => {
    history.replaceState({}, "", "/subscriptions/sub");
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    const user = userEvent.setup();
    const subscription={id:"sub",name:"This Week in Rust",sourceTitle:"This Week in Rust",sourceUrl:"https://example.com/feed",sourceType:"feed" as const,personalNote:"",count:3,unreadCount:2,status:"active" as const};
    const client=mockClient({"/api/subscriptions/sub":subscription});const signOut=vi.spyOn(client,"signOut");
    const view=render(<App client={client}/>);
    await user.type(await screen.findByRole("textbox",{name:"Personal note"}),"keep me");
    await user.click(screen.getByRole("button",{name:"← Back to subscriptions"}));
    expect(window.location.pathname).toBe("/subscriptions/sub");
    history.pushState({},"","/");window.dispatchEvent(new PopStateEvent("popstate"));
    expect(window.location.pathname).toBe("/subscriptions/sub");
    await user.click(screen.getByRole("button",{name:"Account menu"}));await user.click(screen.getByRole("menuitem",{name:"Sign out"}));
    expect(signOut).not.toHaveBeenCalled();expect(confirm).toHaveBeenCalledTimes(3);
    view.unmount();confirm.mockRestore();history.replaceState({},"","/");
  });

  it("uses a generic accessible name for every subscription options menu",async()=>{renderApp();expect(await screen.findAllByRole("button",{name:"More options"})).not.toHaveLength(0);expect(screen.queryByRole("button",{name:"Subscription actions"})).not.toBeInTheDocument()});

  it("preserves the discovered source title when adding a subscription", async () => {
    const calls:{path:string;body?:Record<string,unknown>}[]=[];
    const base=mockClient();
    const client=new ApiClient(async<T,>(path:string,init?:RequestInit)=>{calls.push({path,body:init?.body?JSON.parse(String(init.body)):undefined});if(path==="/api/bootstrap")return await base.bootstrap() as T;if(path==="/api/feeds/discover")return {title:"Canonical source title",kind:"rss",url:"https://example.com/feed",articles:[]} as T;if(path==="/api/subscriptions")return {id:"added",name:"Canonical source title",count:0,unreadCount:0,status:"active"} as T;return undefined as T});
    const user=userEvent.setup();render(<App client={client}/>);await user.click(await screen.findByRole("button",{name:"Add subscription"}));await user.type(screen.getByLabelText("Feed or website URL"),"https://example.com/feed");await user.click(screen.getByRole("button",{name:"Check URL"}));await user.click(await within(screen.getByRole("dialog")).findByRole("button",{name:"Add subscription"}));
    expect(calls.find(call=>call.path==="/api/subscriptions")?.body).toEqual({workspace_id:"ws",url:"https://example.com/feed",title:"Canonical source title"});
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

  it("opens the subscription catalog and stable detail route",async()=>{const user=userEvent.setup();history.replaceState({},"","/");renderApp();await screen.findByRole("heading",{name:"All articles"});await user.click(screen.getByRole("button",{name:"Subscriptions"}));expect(await screen.findByRole("heading",{name:"Subscriptions"})).toBeVisible();expect(location.pathname).toBe("/subscriptions");await user.click(screen.getByRole("link",{name:"This Week in Rust"}));expect(location.pathname).toBe("/subscriptions/sub");history.replaceState({},"","/")});
});
