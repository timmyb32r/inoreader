import { act,render,screen,waitFor } from "@testing-library/preact";
import userEvent from "@testing-library/user-event";
import { App } from "./App";
import { ApiClient,type Bootstrap,type Transport } from "../api/client";
import { articles } from "../test/mockClient";

function recordingClient(){
 const calls:{path:string;init?:RequestInit}[]=[];
 const bootstrap:Bootstrap={account:{displayName:"Test",initials:"T"},workspaces:[{id:"ws",name:"Data engineering",archived:false},{id:"finance",name:"Финансы",archived:false}],activeWorkspaceId:"ws",subscriptions:[{id:"sub",name:"This Week in Rust",count:1,status:"active",lastUpdate:"now"}],articlePage:{articles,total:articles.length,unreadTotal:articles.filter(a=>!a.read&&!a.trash).length}};
 const transport:Transport=async<T,>(path:string,init?:RequestInit)=>{
  calls.push({path,init}); if(path.startsWith("/api/bootstrap"))return bootstrap as T;
  if(path.startsWith("/api/articles?"))return {articles:[{...articles[0],id:"finance",title:"Finance workspace article"}],total:1,unreadTotal:1} as T;
  if(path.startsWith("/api/subscriptions?"))return [] as T;
  if(path.startsWith("/api/rules?"))return [] as T;
  if(path==="/api/opml/import")return {preview_id:"preview",subscriptions:2,warnings:[]} as T;
  if(path==="/api/web-feeds/recipes"){const request=JSON.parse(String(init?.body));return (request.preview?{title:"Preview",kind:"rss",url:"https://test",articles:[{title:"Actual selection"}]}:{id:"web",name:"Web",count:0,status:"active",lastUpdate:"now"}) as T;}
  if(path.includes("/state"))return {...articles[0],...JSON.parse(String(init?.body))} as T;
  return undefined as T;
 };
 return {client:new ApiClient(transport),calls};
}

beforeEach(()=>history.replaceState({},"","/"));

describe("advanced API-backed flows",()=>{
 it("reloads articles when switching workspace",async()=>{const {client,calls}=recordingClient();const user=userEvent.setup();render(<App client={client}/>);await screen.findByRole("heading",{name:"All articles"});await user.click(screen.getByRole("button",{name:/Data engineering/}));await user.click(screen.getByRole("button",{name:/Финансы/}));expect(await screen.findByRole("heading",{name:"Finance workspace article",level:2})).toBeVisible();expect(calls.some(call=>call.path.includes("workspace_id=finance"))).toBe(true);});
 it("gates OPML apply behind preview and deduplicates activation",async()=>{const {client,calls}=recordingClient();const user=userEvent.setup();render(<App client={client}/>);await screen.findByRole("heading",{name:"All articles"});await user.click(screen.getByRole("button",{name:"Settings & shortcuts"}));const apply=screen.getByRole("button",{name:"Apply previewed import"});expect(apply).toBeDisabled();await user.type(screen.getByLabelText("OPML document"),"<opml/>");await user.dblClick(screen.getByRole("button",{name:"Preview import"}));await waitFor(()=>expect(apply).toBeEnabled());expect(calls.filter(call=>call.path==="/api/opml/import")).toHaveLength(1);});
 it("requires a real server preview before Web Feed creation",async()=>{const {client}=recordingClient();const user=userEvent.setup();render(<App client={client}/>);await screen.findByRole("heading",{name:"All articles"});await user.click(screen.getByRole("button",{name:"Add subscription"}));await user.click(screen.getByRole("button",{name:"Build a Web feed"}));const create=screen.getByRole("button",{name:"Create Web feed"});expect(create).toBeDisabled();await user.type(screen.getByLabelText("Public page URL"),"https://test.example");await user.type(screen.getByLabelText("Link selector"),"article");await user.selectOptions(screen.getByLabelText("Loading mode"),"browser");await user.click(screen.getByRole("button",{name:"Preview on server"}));expect(await screen.findByText("Actual selection")).toBeVisible();expect(create).toBeEnabled();});
 it("returns to authentication on unauthorized",async()=>{const {client}=recordingClient();render(<App client={client}/>);await screen.findByRole("heading",{name:"All articles"});act(()=>{window.dispatchEvent(new CustomEvent("reader:unauthorized"));});expect(await screen.findByRole("heading",{name:"Welcome back"})).toBeVisible();});
});

describe("workspace and subscription isolation",()=>{
 it("targets pause at the selected subscription and sends the exact reason",async()=>{
  const calls:{path:string;init?:RequestInit}[]=[];
  const bootstrap:Bootstrap={account:{displayName:"Test",initials:"T"},workspaces:[{id:"ws",name:"Data",archived:false}],activeWorkspaceId:"ws",subscriptions:[{id:"first",name:"Same name",count:1,status:"active",lastUpdate:"now"},{id:"second",name:"Same name",count:1,status:"active",lastUpdate:"now"}],articlePage:{articles:[{...articles[0],subscriptionIds:["second"]}],total:1,unreadTotal:1}};
  const transport:Transport=async<T,>(path:string,init?:RequestInit)=>{calls.push({path,init});if(path.startsWith("/api/bootstrap"))return bootstrap as T;if(path.startsWith("/api/articles?"))return bootstrap.articlePage as T;return undefined as T;};
  const user=userEvent.setup();render(<App client={new ApiClient(transport)}/>);await screen.findByRole("heading",{name:"All articles"});
  const sources=screen.getAllByRole("button",{name:/Same name/});await user.click(sources[1]);await user.click(screen.getByRole("button",{name:"Settings & shortcuts"}));await user.click(screen.getByRole("button",{name:"Pause Same name"}));
  await user.type(screen.getByLabelText(/Reason/),"Exact reason");await user.click(screen.getByRole("button",{name:"Pause subscription"}));
  await waitFor(()=>expect(calls.some(call=>call.path==="/api/subscriptions/second/pause"&&JSON.parse(String(call.init?.body)).reason==="Exact reason")).toBe(true));
 });

 it("scopes mark-all to the selected subscription",async()=>{
  const calls:{path:string;init?:RequestInit}[]=[];
  const bootstrap:Bootstrap={account:{displayName:"Test",initials:"T"},workspaces:[{id:"ws",name:"Data",archived:false}],activeWorkspaceId:"ws",subscriptions:[{id:"chosen",name:"Chosen feed",count:1,status:"active",lastUpdate:"now"}],articlePage:{articles:[{...articles[0],id:"chosen-article",subscriptionIds:["chosen"]},{...articles[1],id:"other-article",subscriptionIds:["other"]}],total:2,unreadTotal:2}};
  const transport:Transport=async<T,>(path:string,init?:RequestInit)=>{calls.push({path,init});if(path.startsWith("/api/bootstrap"))return bootstrap as T;if(path.startsWith("/api/articles?"))return bootstrap.articlePage as T;return undefined as T;};
  const user=userEvent.setup();render(<App client={new ApiClient(transport)}/>);await screen.findByRole("heading",{name:"All articles"});await user.click(screen.getByRole("button",{name:/Chosen feed/}));await user.click(screen.getByRole("button",{name:"Mark all read"}));
  await waitFor(()=>{const call=calls.find(item=>item.path.includes("mark-all-read"));expect(JSON.parse(String(call?.init?.body))).toEqual({view:"all",subscription_id:"chosen"});});
  await user.click(screen.getByRole("button",{name:"All articles"}));expect(screen.getByText("The durable queue is the product").closest("article")).not.toHaveClass("read");
 });
});

describe("subscription lifecycle",()=>{
 it("only offers recipe editing for editable Web feeds",async()=>{
  const bootstrap:Bootstrap={account:{displayName:"Test",initials:"T"},workspaces:[{id:"ws",name:"Data",archived:false}],activeWorkspaceId:"ws",subscriptions:[{id:"rss",name:"RSS",count:0,status:"active",editableWebFeed:false},{id:"web",name:"Web",count:0,status:"active",editableWebFeed:true}],articlePage:{articles:[],total:0,unreadTotal:0}};
  const transport:Transport=async<T,>(path:string)=>path.startsWith("/api/bootstrap")?bootstrap as T:path.startsWith("/api/articles?")?bootstrap.articlePage as T:undefined as T;
  const user=userEvent.setup();render(<App client={new ApiClient(transport)}/>);await screen.findByRole("heading",{name:"All articles"});await user.click(screen.getByRole("button",{name:"Settings & shortcuts"}));
  await user.selectOptions(screen.getByLabelText("Choose subscription"),"rss");expect(screen.queryByRole("button",{name:"Edit Web feed recipe"})).not.toBeInTheDocument();
  await user.selectOptions(screen.getByLabelText("Choose subscription"),"web");expect(screen.getByRole("button",{name:"Edit Web feed recipe"})).toBeVisible();
 });
 it("renames and unsubscribes without removing archived articles, then exposes restore",async()=>{
  const calls:{path:string;init?:RequestInit}[]=[];
  const bootstrap:Bootstrap={account:{displayName:"Test",initials:"T"},workspaces:[{id:"ws",name:"Data",archived:false}],activeWorkspaceId:"ws",subscriptions:[{id:"sub",name:"Original",count:1,status:"active",lastUpdate:"now"}],articlePage:{articles:[{...articles[0],subscriptionIds:["sub"]}],total:1,unreadTotal:1}};
  const transport:Transport=async<T,>(path:string,init?:RequestInit)=>{calls.push({path,init});if(path.startsWith("/api/bootstrap"))return bootstrap as T;if(path.startsWith("/api/articles?"))return bootstrap.articlePage as T;if(path==="/api/subscriptions/sub"&&init?.method==="PATCH")return {...bootstrap.subscriptions[0],name:"Renamed"} as T;if(path==="/api/subscriptions/sub"&&init?.method==="DELETE")return {...bootstrap.subscriptions[0],name:"Renamed",status:"archived"} as T;if(path==="/api/subscriptions/sub/restore")return {...bootstrap.subscriptions[0],name:"Renamed",status:"active"} as T;return undefined as T;};
  const user=userEvent.setup();render(<App client={new ApiClient(transport)}/>);await screen.findByRole("heading",{name:"All articles"});await user.click(screen.getByRole("button",{name:"Settings & shortcuts"}));await user.selectOptions(screen.getByLabelText("Choose subscription"),"sub");const name=screen.getByLabelText("Custom name");await user.clear(name);await user.type(name,"Renamed");await user.click(screen.getByRole("button",{name:"Rename subscription"}));await waitFor(()=>expect(screen.getByRole("button",{name:"Unsubscribe"})).toBeEnabled());await user.click(screen.getByRole("button",{name:"Unsubscribe"}));await user.click(screen.getByRole("button",{name:"Confirm unsubscribe"}));await waitFor(()=>expect(calls.some(call=>call.path==="/api/subscriptions/sub"&&call.init?.method==="DELETE")).toBe(true));expect(await screen.findByText(/Existing articles and full text remain available/)).toBeVisible();expect(screen.getByRole("button",{name:"Restore subscription"})).toBeVisible();await user.click(screen.getByRole("button",{name:"Done"}));expect(screen.getByRole("heading",{name:"Async Rust without the hidden machinery",level:2})).toBeVisible();
 });
});
