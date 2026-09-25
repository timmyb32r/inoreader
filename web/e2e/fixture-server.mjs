import { createServer } from "node:http";
import { readFile, stat } from "node:fs/promises";
import { extname, join, normalize } from "node:path";

const root = join(process.cwd(), "dist");
const articles = [
  { id:"1",url:"https://example.test/rust",source:"This Week in Rust",sources:["This Week in Rust"],subscriptionIds:["sub"],title:"Async Rust without the hidden machinery",excerpt:"Structured concurrency.",body:["A deterministic fixture article."],age:"12 min",read:false,saved:true,later:false,trash:false,fullText:"ready" },
  { id:"2",url:"https://example.test/queue",source:"Database Internals",title:"The durable queue is the product",excerpt:"Retries and intent.",body:["A second deterministic fixture."],age:"43 min",read:false,saved:false,later:false,trash:false,fullText:"failed" },
];
const workspaces=[{id:"ws",name:"Data engineering",archived:false},{id:"finance",name:"Финансы",archived:false}];
let rules=[];
const json=(response,status,body)=>{response.writeHead(status,{"content-type":"application/json"});response.end(JSON.stringify(body));};
const body=async request=>{let value="";for await(const chunk of request)value+=chunk;return value?JSON.parse(value):{};};

createServer(async(request,response)=>{
 const url=new URL(request.url,"http://127.0.0.1:4173");
 if(url.pathname.startsWith("/api/")){
  if(url.pathname==="/api/bootstrap")return json(response,200,{account:{displayName:"Fixture",initials:"FX"},workspaces,activeWorkspaceId:"ws",subscriptions:[{id:"sub",name:"This Week in Rust",count:2,status:"active",lastUpdate:"now"}],articles,newArticleCount:0});
  if(url.pathname==="/api/subscriptions"&&request.method==="GET")return json(response,200,url.searchParams.get("workspace_id")==="finance"?[]:[{id:"sub",name:"This Week in Rust",count:2,status:"active",lastUpdate:"now"}]);
  if(url.pathname==="/api/articles")return json(response,200,url.searchParams.get("workspace_id")==="finance"?[{...articles[0],id:"finance-1",title:"Finance workspace article"}]:articles);
  if(/\/api\/articles\/[^/]+\/state/.test(url.pathname)){const patch=await body(request);const item=articles.find(value=>url.pathname.includes(value.id));return json(response,200,{...item,...patch});}
  if(url.pathname==="/api/feeds/discover")return json(response,200,{title:"Fixture Feed",kind:"rss",url:"https://feed.test",articles:[{title:"Discovered article"}]});
  if(url.pathname==="/api/subscriptions"&&request.method==="POST")return json(response,200,{id:"added",name:"Fixture Feed",count:0,status:"active",lastUpdate:"now"});
  if(url.pathname==="/api/rules"&&request.method==="GET")return json(response,200,rules);
  if(url.pathname==="/api/rules/preview"&&request.method==="POST")return json(response,200,{matchedArticles:4,sharedArticles:2,totalSubscriptionArticles:9,sampleArticleIds:["article-1","article-2"]});
  if(url.pathname==="/api/rules"&&request.method==="POST"){const draft=await body(request);const rule={...draft,id:`rule-${rules.length+1}`};rules.push(rule);return json(response,200,rule);}
  if(/^\/api\/rules\/[^/]+\/apply$/.test(url.pathname)&&request.method==="POST")return json(response,200,{operationId:"operation-1",status:"queued"});
  if(url.pathname==="/api/rule-applications/operation-1")return json(response,200,{operationId:"operation-1",status:"completed",evaluated:9});
  if(url.pathname.startsWith("/api/rules/")&&request.method==="DELETE"){rules=rules.filter(rule=>!url.pathname.endsWith(rule.id));response.writeHead(204);return response.end();}
  if(url.pathname==="/api/web-feeds/recipes"){const draft=await body(request);if(draft.preview)return json(response,200,{title:"Server selection",kind:"rss",url:draft.url,articles:[{title:"Selected from live DOM",publishedAt:"2026-09-25"}]});return json(response,200,{id:"web",name:"Server selection",count:0,status:"active",lastUpdate:"now"});}
  if(url.pathname==="/api/opml/import"){const input=await body(request);return json(response,200,{preview_id:"opml-preview",subscriptions:input.apply?2:2,warnings:[]});}
  if(url.pathname==="/api/opml/export")return json(response,200,"<opml version=\"2.0\"/>");
  if(url.pathname==="/api/workspaces"&&request.method==="POST"){const input=await body(request);return json(response,200,{id:"new-workspace",name:input.name,archived:false});}
  if(url.pathname==="/api/auth/invites")return json(response,200,{url:"https://reader.test/invite?token=fixed",expires_at:"2026-09-26T00:00:00Z"});
  if(url.pathname==="/api/auth/password-resets")return json(response,200,{url:"https://reader.test/reset-password?token=fixed",expires_at:"2026-09-26T00:00:00Z"});
  response.writeHead(204);return response.end();
 }
 let file=normalize(join(root,url.pathname==="/"?"index.html":url.pathname));try{if(!(await stat(file)).isFile())file=join(root,"index.html");}catch{file=join(root,"index.html");}
 const content=await readFile(file);const types={".html":"text/html",".js":"text/javascript",".css":"text/css"};response.writeHead(200,{"content-type":types[extname(file)]??"application/octet-stream"});response.end(content);
}).listen(4173,"127.0.0.1",()=>console.log("fixture server http://127.0.0.1:4173"));
