import { ApiClient, type Transport } from "./client";

it("uses the reserved server routes and exact request shapes", async () => {
  const calls: { path:string; init?:RequestInit }[]=[];
  const transport:Transport=async <T>(path:string,init?:RequestInit)=>{calls.push({path,init});return {id:"created"} as T;};
  const client=new ApiClient(transport);
  await client.signIn("reader","secret");
  await client.addSubscription("workspace-id","https://example.com/feed","Discovered title");
  await client.importOpml("workspace-id","<opml/>");
  await client.importOpml("workspace-id","<opml/>","preview-id");
  await client.saveRule("workspace-id",{subscriptionId:"sub",field:"title",phrase:"career",action:"mark_read",enabled:true});
  expect(calls.map(call=>call.path)).toEqual(["/api/auth/sessions","/api/subscriptions","/api/opml/import","/api/opml/import","/api/rules?workspace_id=workspace-id"]);
  expect(JSON.parse(String(calls[1].init?.body))).toEqual({workspace_id:"workspace-id",url:"https://example.com/feed",title:"Discovered title"});
  expect(JSON.parse(String(calls[2].init?.body))).toEqual({workspace_id:"workspace-id",opml:"<opml/>",apply:false});
  expect(JSON.parse(String(calls[3].init?.body))).toEqual({workspace_id:"workspace-id",opml:"<opml/>",apply:true,preview_id:"preview-id"});
});

it("exposes every account password flow with closed payloads",async()=>{
 const calls:{path:string;body:unknown}[]=[];const transport:Transport=async <T>(path:string,init?:RequestInit)=>{calls.push({path,body:JSON.parse(String(init?.body??"null"))});return undefined as T;};const client=new ApiClient(transport);
 await client.acceptInvite("token","reader","secret");await client.changePassword("old","new");await client.resetPassword("reset","newer");
 expect(calls).toEqual([
  {path:"/api/auth/invites/accept",body:{token:"token",username:"reader",password:"secret"}},
  {path:"/api/auth/password/change",body:{current_password:"old",new_password:"new"}},
  {path:"/api/auth/password/reset",body:{token:"reset",new_password:"newer"}},
 ]);
});

it("uses explicit reversible subscription lifecycle routes",async()=>{
 const calls:{path:string;method?:string;body:unknown}[]=[];const transport:Transport=async<T,>(path:string,init?:RequestInit)=>{calls.push({path,method:init?.method,body:init?.body?JSON.parse(String(init.body)):undefined});return {id:"sub"} as T;};const client=new ApiClient(transport);
 await client.renameSubscription("sub","\u6211\u7684\u8ba2\u9605");await client.unsubscribe("sub");await client.restoreSubscription("sub");
 expect(calls).toEqual([
  {path:"/api/subscriptions/sub",method:"PATCH",body:{name:"\u6211\u7684\u8ba2\u9605"}},
  {path:"/api/subscriptions/sub",method:"DELETE",body:undefined},
  {path:"/api/subscriptions/sub/restore",method:"POST",body:undefined},
 ]);
});
