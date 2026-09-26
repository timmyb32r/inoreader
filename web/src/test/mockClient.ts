import { ApiClient, type Bootstrap, type Transport } from "../api/client";
import type { Article } from "../app/data";

export const articles: Article[] = [
  { id:"1",url:"https://example.com/rust",source:"This Week in Rust",sources:["This Week in Rust"],subscriptionIds:["sub"],title:"Async Rust without the hidden machinery",excerpt:"Structured concurrency.",body:["Body"],age:"12 min",read:false,saved:true,later:false,trash:false,fullText:"ready" },
  { id:"2",url:"https://example.com/queues",source:"Database Internals",title:"The durable queue is the product",excerpt:"Retries and intent.",body:["Body"],age:"43 min",read:false,saved:false,later:false,trash:false,fullText:"ready" },
  { id:"3",url:"https://example.com/data",source:"数据工程周刊",title:"流式系统中的背压设计",excerpt:"背压设计。",body:["正文"],age:"1 hr",read:false,saved:false,later:false,trash:false,fullText:"ready" },
];
const bootstrap: Bootstrap = { account:{displayName:"Test",initials:"TB"},workspaces:[{id:"ws",name:"Data engineering",archived:false},{id:"finance",name:"Финансы",archived:false}],activeWorkspaceId:"ws",subscriptions:[{id:"sub",name:"This Week in Rust",count:3,status:"active",lastUpdate:"now"}],articlePage:{articles,total:articles.length,unreadTotal:articles.filter(a=>!a.read&&!a.trash).length} };

export function mockClient(overrides: Record<string, unknown> = {}) {
  const transport: Transport = async <T>(path: string, init?: RequestInit) => {
    if (path.startsWith("/api/bootstrap")) return bootstrap as T;
    if (path.startsWith("/api/articles?")) return bootstrap.articlePage as T;
    if (path === "/api/subscriptions?workspace_id=finance") return [] as T;
    if (path.includes("/state")) { const id=path.split("/").at(-2); return {...articles.find(a=>a.id===id),...JSON.parse(String(init?.body??"{}"))} as T; }
    if (path in overrides) { const value=overrides[path]; if (value instanceof Error) throw value; return value as T; }
    return undefined as T;
  };
  return new ApiClient(transport);
}
