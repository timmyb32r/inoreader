import type { Article, Subscription, Workspace } from "../app/data";
import { reportApiRequest } from "../performanceDiagnostics";

export type Bootstrap = { account: { displayName: string; initials: string }; workspaces: Workspace[]; activeWorkspaceId: string; subscriptions: Subscription[]; articles: Article[]; newArticleCount: number };
export type RuleDraft = { id?: string; subscriptionId: string; field: "title" | "full_text" | "title_or_full_text"; phrase: string; action: "mark_read" | "move_to_trash"; enabled: boolean };
export type RulePreview = { matchedArticles:number; sharedArticles:number; totalSubscriptionArticles:number; sampleArticleIds:string[] };
export type RuleApplicationStatusName = "queued"|"running"|"completed"|"cancelled"|"failed";
export type RuleApplicationAccepted = { operationId:string; status:RuleApplicationStatusName };
export type RuleApplicationStatus = RuleApplicationAccepted & { evaluated:number; cancelReason?:string };
export type FeedPreview = { title: string; kind: "rss" | "atom" | "json_feed" | "web_feed"; url: string; articles: { title: string; publishedAt?: string }[] };
export type SelectorDraft = { language: "css" | "xpath"; expression: string };
export type WebFeedDraft = {
  workspaceId: string;
  url: string;
  selector: string;
  loading: "automatic" | "static" | "browser";
  selectorLanguage: SelectorDraft["language"];
  viewport: "desktop" | "mobile";
  listingUrl?: string;
  cardSelector?: string;
  titleSelector?: string;
  dateSelector?: string;
  contentSelector?: string;
  waitSelector?: string;
  urlPattern?: string;
  maxPages: number;
  hideOverlays: SelectorDraft[];
  startPages: string[];
  nextPage?: SelectorDraft;
  loadMore?: SelectorDraft;
  loadMoreClicks: number;
  scrolls: number;
};
export type WebFeedRecipeView={subscriptionId:string;version:number;draft:WebFeedDraft};
export type VisualRect = { x:number; y:number; width:number; height:number };
export type VisualCandidateGroup = { id:string; selector:SelectorDraft; count:number; boxes:VisualRect[] };
export type VisualPreview = { snapshotToken:string; expiresAt:string; imageDataUrl:string; width:number; height:number; groups:VisualCandidateGroup[] };
export type VisualSelection = { selector:SelectorDraft; count:number; similarItems:VisualRect[] };
export type OpmlPreview = { preview_id: string; subscriptions: number; warnings: string[] };
export type SubscriptionActivity = { id:string; occurredAt:string; successful:boolean; durationMs?:number; discoveredItems?:number; diagnostic?:string };
export type SourceUrlPreview = { token:string; url:string; title:string; expiresAt:string };
export type SubscriptionExtraction = { sourceType:string; feedUrls:string[]; recipeVersion?:number; recipeSummary?:string; lastPreview?:string };
export type SubscriptionRuleView = { id:string; scope:"subscription"|"workspace"; summary:string; enabled:boolean };
export type SubscriptionDetail = Subscription & { feedUrls?:string[]; extractionSummary?:string; lastPreview?:string; activity?:SubscriptionActivity[]; rules?:SubscriptionRuleView[] };
export type Transport = <T>(path: string, init?: RequestInit) => Promise<T>;

export class ApiError extends Error { constructor(readonly status: number, message: string) { super(message); } }

export class ApiClient {
  constructor(private readonly transport: Transport) {}
  bootstrap = () => this.transport<Bootstrap>("/api/bootstrap");
  signIn = (username: string, password: string) => this.transport<void>("/api/auth/sessions", json("POST", { username, password }));
  signOut = () => this.transport<void>("/api/auth/sessions", { method: "DELETE" });
  acceptInvite = (token: string, username: string, password: string) => this.transport<void>("/api/auth/invites/accept", json("POST", { token, username, password }));
  changePassword = (currentPassword: string, newPassword: string) => this.transport<void>("/api/auth/password/change", json("POST", { current_password: currentPassword, new_password: newPassword }));
  resetPassword = (token: string, newPassword: string) => this.transport<void>("/api/auth/password/reset", json("POST", { token, new_password: newPassword }));
  listArticles = (workspaceId: string, view: string, subscriptionId?: string) => this.transport<Article[]>(`/api/articles?workspace_id=${enc(workspaceId)}&view=${enc(view)}${subscriptionId ? `&subscription_id=${enc(subscriptionId)}` : ""}`);
  getArticle = (workspaceId: string, articleId: string) => this.transport<Article>(`/api/articles/${enc(articleId)}?workspace_id=${enc(workspaceId)}`);
  listSubscriptions = (workspaceId: string) => this.transport<Subscription[]>(`/api/subscriptions?workspace_id=${enc(workspaceId)}`);
  getSubscription = (id:string) => this.transport<SubscriptionDetail>(`/api/subscriptions/${enc(id)}`);
  saveSubscriptionNote = (id:string,note:string) => this.transport<SubscriptionDetail>(`/api/subscriptions/${enc(id)}/note`,json("PUT",{note}));
  subscriptionActivity = (id:string) => this.transport<SubscriptionActivity[]>(`/api/subscriptions/${enc(id)}/activity`);
  subscriptionExtraction = (id:string) => this.transport<SubscriptionExtraction>(`/api/subscriptions/${enc(id)}/extraction`);
  previewSubscriptionSourceUrl = (id:string,url:string) => this.transport<SourceUrlPreview>(`/api/subscriptions/${enc(id)}/source-url/preview`,json("POST",{url}));
  commitSubscriptionSourceUrl = (id:string,previewToken:string) => this.transport<Subscription>(`/api/subscriptions/${enc(id)}/source-url`,json("PUT",{previewToken}));
  updateArticle = (workspaceId: string, articleId: string, state: Partial<Pick<Article,"read"|"saved"|"later"|"trash">>) => this.transport<Article>(`/api/articles/${enc(articleId)}/state?workspace_id=${enc(workspaceId)}`, json("POST", state));
  markAllRead = (workspaceId: string, view: string, subscriptionId?: string) => this.transport<void>(`/api/workspaces/${enc(workspaceId)}/articles/mark-all-read`, json("POST", { view, subscription_id: subscriptionId }));
  renameWorkspace = (id: string, name: string) => this.transport<Workspace>(`/api/workspaces/${enc(id)}`, json("PATCH", { name }));
  createWorkspace = (name: string) => this.transport<Workspace>("/api/workspaces", json("POST", { name }));
  archiveWorkspace = (id: string, reason: string) => this.transport<void>(`/api/workspaces/${enc(id)}/archive`, json("POST", { reason }));
  restoreWorkspace = (id: string) => this.transport<void>(`/api/workspaces/${enc(id)}/restore`, { method: "POST" });
  discoverFeed = (url: string) => this.transport<FeedPreview>("/api/feeds/discover", json("POST", { url }));
  addSubscription = (workspaceId: string, url: string, title?: string) => this.transport<Subscription>("/api/subscriptions", json("POST", { workspace_id: workspaceId, url, title }));
  refreshSubscription = (id: string) => this.transport<void>(`/api/subscriptions/${enc(id)}/refresh`, { method: "POST" });
  refreshFullText = (workspaceId: string, articleId: string) => this.transport<void>(`/api/articles/${enc(articleId)}/full-text/refresh?workspace_id=${enc(workspaceId)}`, { method: "POST" });
  pauseSubscription = (id: string, reason: string) => this.transport<void>(`/api/subscriptions/${enc(id)}/pause`, json("POST", { reason }));
  resumeSubscription = (id: string) => this.transport<void>(`/api/subscriptions/${enc(id)}/resume`, { method: "POST" });
  renameSubscription = (id: string, name: string) => this.transport<Subscription>(`/api/subscriptions/${enc(id)}`, json("PATCH", { name }));
  unsubscribe = (id: string) => this.transport<Subscription>(`/api/subscriptions/${enc(id)}`, { method: "DELETE" });
  restoreSubscription = (id: string) => this.transport<Subscription>(`/api/subscriptions/${enc(id)}/restore`, { method: "POST" });
  listRules = (workspaceId: string) => this.transport<RuleDraft[]>(`/api/rules?workspace_id=${enc(workspaceId)}`);
  saveRule = (workspaceId: string, rule: RuleDraft) => this.transport<RuleDraft>(`/api/rules${rule.id ? `/${enc(rule.id)}` : ""}?workspace_id=${enc(workspaceId)}`, json(rule.id ? "PUT" : "POST", rule));
  previewRule = (workspaceId:string,rule:RuleDraft) => this.transport<RulePreview>(`/api/rules/preview?workspace_id=${enc(workspaceId)}`,json("POST",rule));
  applyRule = (workspaceId: string, id: string) => this.transport<RuleApplicationAccepted>(`/api/rules/${enc(id)}/apply?workspace_id=${enc(workspaceId)}`, { method: "POST" });
  ruleApplicationStatus = (workspaceId:string,operationId:string) => this.transport<RuleApplicationStatus>(`/api/rule-applications/${enc(operationId)}?workspace_id=${enc(workspaceId)}`);
  deleteRule = (workspaceId: string, id: string) => this.transport<void>(`/api/rules/${enc(id)}?workspace_id=${enc(workspaceId)}`, { method: "DELETE" });
  importOpml = (workspaceId: string, document: string, previewId?: string) => this.transport<OpmlPreview>("/api/opml/import", json("POST", { workspace_id: workspaceId, opml: document, apply: previewId !== undefined, preview_id: previewId }));
  exportOpml = (workspaceId: string) => this.transport<string>(`/api/opml/export?workspace_id=${enc(workspaceId)}`);
  previewWebFeed = (draft: WebFeedDraft) => this.transport<FeedPreview>("/api/web-feeds/recipes", json("POST", { ...draft, preview: true }));
  createWebFeed = (draft: WebFeedDraft) => this.transport<Subscription>("/api/web-feeds/recipes", json("POST", draft));
  getWebFeedRecipe=(subscriptionId:string)=>this.transport<WebFeedRecipeView>(`/api/web-feeds/recipes/${enc(subscriptionId)}`);
  updateWebFeedRecipe=(subscriptionId:string,expectedVersion:number,draft:WebFeedDraft)=>this.transport<WebFeedRecipeView>(`/api/web-feeds/recipes/${enc(subscriptionId)}`,json("PUT",{expectedVersion,draft}));
  visualPreview = (workspaceId:string,url:string,viewport:WebFeedDraft["viewport"]) => this.transport<VisualPreview>("/api/web-feeds/visual-previews",json("POST",{workspaceId,url,viewport}));
  visualSelect = (workspaceId:string,snapshotToken:string,x:number,y:number) => this.transport<VisualSelection>("/api/web-feeds/visual-selections",json("POST",{workspaceId,snapshotToken,x,y}));
  createInvite = (username: string) => this.transport<{ url:string; expires_at:string }>("/api/auth/invites", json("POST", { username }));
  createPasswordReset = (username: string) => this.transport<{ url:string; expires_at:string }>("/api/auth/password-resets", json("POST", { username }));
}

export const fetchTransport: Transport = async <T>(path: string, init?: RequestInit) => {
  const method = init?.method ?? "GET";
  const startedAt = performance.now();
  let status:number|"network-error"="network-error";
  try {
    const response = await fetch(path, { credentials: "same-origin", headers: { Accept: "application/json", ...init?.headers }, ...init });
    status=response.status;
    if (response.status === 401) window.dispatchEvent(new CustomEvent("reader:unauthorized"));
    if (!response.ok) { let message = `Request failed (${response.status})`; try { const body = await response.json() as { message?: string }; if (body.message) message = body.message; } catch { /* response may be empty */ } throw new ApiError(response.status, message); }
    if (response.status === 204) return undefined as T;
    return await response.json() as T;
  } finally {
    reportApiRequest(path,method,startedAt,status);
  }
};
export const apiClient = new ApiClient(fetchTransport);
const enc = encodeURIComponent;
const json = (method: string, body: unknown): RequestInit => ({ method, headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) });
