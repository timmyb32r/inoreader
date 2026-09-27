import type { Transport } from "./client";

export type {
  AiProfile,
  ChatMessage,
  ArticleChat,
  TranslationSegment,
  ParagraphTranslation,
  ParagraphJob,
} from "./generated";
import type { AiProfile, ArticleChat, ParagraphJob } from "./generated";
export type {
  Balance as AiBalance,
  GenerationPhase as ChatPhase,
  ProviderCall as ChatProviderCall,
} from "./generated";

export function isChatActive(chat: ArticleChat | null): boolean {
  return (
    !!chat &&
    ["waiting_content", "queued", "generating", "verifying"].includes(
      chat.status,
    )
  );
}

/** Keys are sent only in an explicit mutation body, never URLs or browser storage. */
export class AiClient {
  constructor(private readonly transport: Transport) {}
  translations = (workspaceId: string, articleId: string) =>
    this.transport<ParagraphJob[]>(
      `/api/articles/${encodeURIComponent(articleId)}/translations?workspace_id=${encodeURIComponent(workspaceId)}`,
      undefined,
      "ParagraphJob[]",
    );
  translate = (
    workspaceId: string,
    articleId: string,
    operationId: string,
    source: string,
  ) =>
    this.transport<ParagraphJob>(
      `/api/articles/${encodeURIComponent(articleId)}/translations`,
      json("POST", { workspaceId, operationId, source }),
      "ParagraphJob",
    );
  profile = () =>
    this.transport<AiProfile>("/api/ai/profile", undefined, "AiProfile");
  saveKey = (apiKey: string) =>
    this.transport<AiProfile>(
      "/api/ai/profile",
      json("PUT", { apiKey }),
      "AiProfile",
    );
  removeKey = () =>
    this.transport<AiProfile>(
      "/api/ai/profile",
      { method: "DELETE" },
      "AiProfile",
    );
  balance = () =>
    this.transport<AiProfile>(
      "/api/ai/balance",
      { method: "POST" },
      "AiProfile",
    );
  versions = (workspaceId: string, articleId: string) =>
    this.transport<ArticleChat[]>(
      `/api/articles/${encodeURIComponent(articleId)}/chats?workspace_id=${encodeURIComponent(workspaceId)}`,
      undefined,
      "ArticleChat[]",
    );
  open = (
    workspaceId: string,
    articleId: string,
    operationId: string,
    regenerate = false,
  ) =>
    this.transport<ArticleChat>(
      `/api/articles/${encodeURIComponent(articleId)}/chat`,
      json("POST", { workspaceId, operationId, regenerate }),
      "ArticleChat",
    );
  get = (id: string) =>
    this.transport<ArticleChat>(
      `/api/ai/chats/${encodeURIComponent(id)}`,
      undefined,
      "ArticleChat",
    );
  send = (id: string, operationId: string, content: string) =>
    this.transport<ArticleChat>(
      `/api/ai/chats/${encodeURIComponent(id)}/messages`,
      json("POST", { operationId, content }),
      "ArticleChat",
    );
  stop = (id: string) =>
    this.transport<ArticleChat>(
      `/api/ai/chats/${encodeURIComponent(id)}/stop`,
      { method: "POST" },
      "ArticleChat",
    );
  retry = (id: string, operationId: string) =>
    this.transport<ArticleChat>(
      `/api/ai/chats/${encodeURIComponent(id)}/retry`,
      json("POST", { operationId }),
      "ArticleChat",
    );
}

const json = (method: string, body: unknown): RequestInit => ({
  method,
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify(body),
});
