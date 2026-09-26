export type Article = {
  id: string;
  url: string;
  source: string;
  sources?: string[];
  subscriptionIds?: string[];
  title: string;
  excerpt: string;
  body: string[];
  bodyHtml?: string;
  author?: string;
  age: string;
  read: boolean;
  saved: boolean;
  later: boolean;
  trash: boolean;
  fullText: "ready" | "pending" | "failed";
  fullTextReason?: string;
  originalUrl?: string;
};

export type Subscription = { id: string; name: string; sourceTitle?: string; customName?: string; sourceUrl?: string; personalNote?: string; sourceType?: "feed"|"web"|"built_in"; count: number; unreadCount?: number; status: "active" | "paused" | "archived"; lastUpdate?: string; lastErrorAt?:string; createdAt?:string; pollingInterval?:string; incomplete?:boolean; continuation?:string; error?:string; needsAttention?:boolean; attentionReason?:string; editableWebFeed?:boolean; reason?: string; reasonAt?: string };
export type Workspace = { id: string; name: string; archived: boolean; archiveReason?: string; archiveReasonAt?: string };
