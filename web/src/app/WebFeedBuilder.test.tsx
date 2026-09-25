import { fireEvent, render, screen, waitFor } from "@testing-library/preact";
import userEvent from "@testing-library/user-event";
import { ApiClient, type Transport } from "../api/client";
import { WebFeedBuilder } from "./WebFeedBuilder";

function setup(deferred = false) {
  const requests: Record<string, unknown>[] = [];
  let release: (() => void) | undefined;
  const hold = new Promise<void>(resolve => { release = resolve; });
  const transport: Transport = async <T,>(_path: string, init?: RequestInit) => {
    requests.push(JSON.parse(String(init?.body)) as Record<string, unknown>);
    if (deferred) await hold;
    const request = requests.at(-1)!;
    return (request.preview ? { title: "Selected stories", kind: "web_feed", url: request.url, articles: [{ title: "One" }] } : { id: "web", name: "Selected stories", count: 0, status: "active", lastUpdate: "now" }) as T;
  };
  render(<WebFeedBuilder client={new ApiClient(transport)} workspaceId="workspace" onClose={() => undefined} onDone={() => undefined}/>);
  return { requests, release: () => release?.() };
}

async function required(user: ReturnType<typeof userEvent.setup>) {
  await user.type(screen.getByLabelText("Public page URL"), "https://example.test/news");
  await user.type(screen.getByLabelText("Link selector"), "//article");
  await user.selectOptions(screen.getByLabelText("Link selector language"), "xpath");
}

describe("WebFeedBuilder", () => {
  it("prefills a versioned recipe and sends an optimistic update after preview",async()=>{
    const calls:{path:string;body:Record<string,unknown>}[]=[];const transport:Transport=async<T,>(path:string,init?:RequestInit)=>{const body=JSON.parse(String(init?.body));calls.push({path,body});if(body.preview)return {title:"Preview",kind:"web_feed",url:body.url,articles:[{title:"One"}]} as T;return {subscriptionId:"sub",version:5,draft:body.draft} as T};
    const draft={workspaceId:"workspace",url:"https://example.test/news",selector:"a.story",loading:"browser" as const,selectorLanguage:"css" as const,viewport:"desktop" as const,listingUrl:"https://example.test/archive",cardSelector:"article",titleSelector:"h2",dateSelector:"time",contentSelector:".body",waitSelector:"main",urlPattern:"/story/",maxPages:3,hideOverlays:[],startPages:[],loadMoreClicks:0,scrolls:0};
    render(<WebFeedBuilder client={new ApiClient(transport)} workspaceId="workspace" editing={{subscriptionId:"sub",version:4,draft}} onClose={()=>undefined} onDone={()=>undefined} onUpdated={()=>undefined}/>);const user=userEvent.setup();expect(screen.getByLabelText("Public page URL")).toHaveValue(draft.url);expect(screen.getByLabelText("Card/container selector")).toHaveValue("article");await user.click(screen.getByRole("button",{name:"Preview on server"}));await screen.findByText("One");await user.click(screen.getByRole("button",{name:"Save recipe"}));await waitFor(()=>expect(calls.at(-1)?.path).toBe("/api/web-feeds/recipes/sub"));expect(calls.at(-1)?.body).toMatchObject({expectedVersion:4,draft:{maxPages:3,cardSelector:"article"}});
  });
  it("uses an opaque visual snapshot to validate a detected repeated group", async () => {
    const requests:string[]=[];const transport:Transport=async<T,>(path:string,init?:RequestInit):Promise<T>=>{requests.push(path);const body=JSON.parse(String(init?.body));if(path.endsWith("visual-previews"))return {snapshotToken:"opaque",expiresAt:"2099-01-01T00:00:00Z",imageDataUrl:"data:image/png;base64,iVBORw0KGgo=",width:100,height:100,groups:[{id:"g",selector:{language:"css",expression:"article.card"},count:4,boxes:[{x:10,y:10,width:20,height:20}]}]} as T;if(path.endsWith("visual-selections")){expect(body).toMatchObject({workspaceId:"workspace",snapshotToken:"opaque",x:20,y:20});return {selector:{language:"css",expression:"article.card"},count:4,similarItems:[]} as T;}throw new Error("unexpected request")};
    render(<WebFeedBuilder client={new ApiClient(transport)} workspaceId="workspace" onClose={()=>undefined} onDone={()=>undefined}/>);const user=userEvent.setup();await user.type(screen.getByLabelText("Public page URL"),"https://example.test/news");await user.click(screen.getByRole("button",{name:"Select from page image"}));await user.click(await screen.findByRole("button",{name:"4 × article.card"}));expect(screen.getByLabelText("Link selector")).toHaveValue("article.card");expect(screen.getByText("Validated repeated group: 4 similar items.")).toBeVisible();expect(requests).toEqual(["/api/web-feeds/visual-previews","/api/web-feeds/visual-selections"]);
  });

  it("sends the complete advanced recipe and gates creation behind its exact preview", async () => {
    const { requests } = setup(); const user = userEvent.setup(); await required(user);
    await user.selectOptions(screen.getByLabelText("Loading mode"), "browser");
    await user.selectOptions(screen.getByLabelText("Viewport"), "mobile");
    await user.click(screen.getByText("Browser actions"));
    await user.click(screen.getByRole("button", { name: "Add overlay selector" }));
    await user.type(screen.getByLabelText("Overlay 1"), ".consent");
    await user.click(screen.getByRole("button", { name: "Add start page" }));
    await user.type(screen.getByLabelText("Start page 1"), "https://example.test/archive");
    fireEvent.input(screen.getByLabelText("Next page selector"), { target: { value: "//a[@rel='next']" } });
    await user.selectOptions(screen.getByLabelText("Next page selector language"), "xpath");
    await user.type(screen.getByLabelText("Load more selector"), "button.more");
    await user.clear(screen.getByLabelText("Load more clicks")); await user.type(screen.getByLabelText("Load more clicks"), "2");
    await user.clear(screen.getByLabelText("Full-page scrolls")); await user.type(screen.getByLabelText("Full-page scrolls"), "3");
    await user.click(screen.getByRole("button", { name: "Preview on server" }));
    expect(await screen.findByText("One")).toBeVisible();
    expect(requests[0]).toMatchObject({ workspaceId: "workspace", selectorLanguage: "xpath", loading: "browser", viewport: "mobile", hideOverlays: [{ language: "css", expression: ".consent" }], startPages: ["https://example.test/archive"], nextPage: { language: "xpath", expression: "//a[@rel='next']" }, loadMore: { language: "css", expression: "button.more" }, loadMoreClicks: 2, scrolls: 3, preview: true });
    expect(screen.getByRole("button", { name: "Create Web feed" })).toBeEnabled();
    fireEvent.input(screen.getByLabelText("Link selector"), { target: { value: "//article[featured]" } });
    await waitFor(()=>expect(screen.getByRole("button", { name: "Create Web feed" })).toBeDisabled());
    expect(screen.getByText("Recipe changed")).toBeVisible();
  });

  it("rejects browser-only settings in static mode before a request", async () => {
    const { requests } = setup(); const user = userEvent.setup(); await required(user);
    await user.selectOptions(screen.getByLabelText("Loading mode"), "static");
    expect(screen.getByText(/Static HTML cannot run XPath/)).toBeVisible();
    expect(screen.getByRole("button", { name: "Preview on server" })).toBeDisabled();
    expect(requests).toHaveLength(0);
  });

  it("shows pending feedback synchronously and deduplicates double activation", async () => {
    const { requests, release } = setup(true); const user = userEvent.setup();
    await user.type(screen.getByLabelText("Public page URL"), "https://example.test"); await user.type(screen.getByLabelText("Link selector"), "article");
    const preview = screen.getByRole("button", { name: "Preview on server" }); await user.dblClick(preview);
    expect(screen.getByRole("button", { name: "Previewing…" })).toBeDisabled(); expect(requests).toHaveLength(1);
    release(); await waitFor(() => expect(screen.getByText("One")).toBeVisible());
  });

  it("discards a preview response when the recipe changes in flight",async()=>{
    const { release }=setup(true);const user=userEvent.setup();await user.type(screen.getByLabelText("Public page URL"),"https://example.test");await user.type(screen.getByLabelText("Link selector"),"article");
    await user.click(screen.getByRole("button",{name:"Preview on server"}));await user.type(screen.getByLabelText("Link selector"),".featured");release();
    await waitFor(()=>expect(screen.getByRole("button",{name:"Preview on server"})).toBeEnabled());expect(screen.queryByText("One")).not.toBeInTheDocument();expect(screen.getByRole("button",{name:"Create Web feed"})).toBeDisabled();
  });
});
