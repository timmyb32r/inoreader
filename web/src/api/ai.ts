import type { Transport } from "./client";

export type AiBalance = {
  available: boolean;
  balances: { currency: string; total: string; granted: string; toppedUp: string }[];
  updatedAt: string;
};
export type AiProfile = { configured: boolean; enabled: boolean; balance?: AiBalance; availabilityReason?: string; error?: string };
export type ChatPhase = "generating" | "verifying";
export type ChatMessage = {
  id: string;
  role: "user" | "assistant";
  content: string;
  status: "pending" | "streaming" | "complete" | "interrupted" | "failed";
  createdAt: string;
  purpose?: "summary" | "chat";
  phase?: ChatPhase;
};
/** Each entry is one provider request, including unsuccessful and retried attempts.
 * Absent usage/cost means unknown, not zero. Money is an exact decimal string. */
export type ChatProviderCall = {
  id: string;
  assistantId: string;
  phase: ChatPhase;
  status: "started" | "completed" | "failed" | "interrupted" | "cancelled";
  usage?: {
    promptTokens: number;
    completionTokens: number;
    promptCacheHitTokens: number;
    promptCacheMissTokens: number;
    estimatedCostUsd?: string;
  };
};
export type ArticleChat = {
  id: string;
  articleId: string;
  workspaceId: string;
  title: string;
  sourceUrl: string;
  createdAt: string;
  model: string;
  promptVersion: string;
  status: "waiting_content" | "queued" | "generating" | "verifying" | "completed" | "failed" | "interrupted" | "cancelled";
  messages: ChatMessage[];
  providerCalls: ChatProviderCall[];
  error?: string;
};

export function isChatActive(chat: ArticleChat | null): boolean {
  return !!chat && ["waiting_content", "queued", "generating", "verifying"].includes(chat.status);
}

/** Keys are sent only in an explicit mutation body, never URLs or browser storage. */
export class AiClient {
  constructor(private readonly transport: Transport) {}
  translations = (workspaceId:string, articleId:string) => this.transport<ParagraphJob[]>(`/api/articles/${encodeURIComponent(articleId)}/translations?workspace_id=${encodeURIComponent(workspaceId)}`);
  translate = (workspaceId:string, articleId:string, operationId:string, source:string) => this.transport<ParagraphJob>(`/api/articles/${encodeURIComponent(articleId)}/translations`,json("POST",{workspaceId,operationId,source}));
  profile = () => this.transport<AiProfile>("/api/ai/profile");
  saveKey = (apiKey: string) => this.transport<AiProfile>("/api/ai/profile", json("PUT", { apiKey }));
  removeKey = () => this.transport<AiProfile>("/api/ai/profile", { method: "DELETE" });
  balance = () => this.transport<AiProfile>("/api/ai/balance", { method: "POST" });
  versions = (workspaceId: string, articleId: string) => this.transport<ArticleChat[]>(`/api/articles/${encodeURIComponent(articleId)}/chats?workspace_id=${encodeURIComponent(workspaceId)}`);
  open = (workspaceId: string, articleId: string, operationId: string, regenerate = false) => this.transport<ArticleChat>(`/api/articles/${encodeURIComponent(articleId)}/chat`, json("POST", { workspaceId, operationId, regenerate }));
  get = (id: string) => this.transport<ArticleChat>(`/api/ai/chats/${encodeURIComponent(id)}`);
  send = (id: string, operationId: string, content: string) => this.transport<ArticleChat>(`/api/ai/chats/${encodeURIComponent(id)}/messages`, json("POST", { operationId, content }));
  stop = (id: string) => this.transport<ArticleChat>(`/api/ai/chats/${encodeURIComponent(id)}/stop`, { method: "POST" });
  retry = (id: string, operationId: string) => this.transport<ArticleChat>(`/api/ai/chats/${encodeURIComponent(id)}/retry`, json("POST", { operationId }));
}

const json = (method: string, body: unknown): RequestInit => ({ method, headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) });

export type TranslationSegment = { kind:"word"; source:string; pinyin:string|null; translation:string } | { kind:"literal"; source:string };
export type ParagraphTranslation = {source:string; translation:string; segments:TranslationSegment[]};
export type ParagraphJob = {id:string;workspaceId:string;articleId:string;source:string;model:string} & ({status:"queued"|"generating"}|{status:"completed";result:ParagraphTranslation}|{status:"failed";error:string});
