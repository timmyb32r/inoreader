import type { Article, Subscription, Workspace } from "../app/data";
import { reportApiRequest } from "../performanceDiagnostics";
import { AiClient } from "./ai";
import { decodeResponse, type ResponseContract } from "./decode";
import type * as Wire from "./generated";
import { GlossaryClient } from "./glossary";

export type ArticlePage = Pick<
  Wire.ArticlePageView,
  "total" | "unreadTotal"
> & { articles: Article[]; newerCursor?: string; olderCursor?: string };
export type ArticlePagePosition = {
  view?: string;
  subscriptionId?: string | null;
  cursor?: string;
  direction?: "older" | "newer";
};
export type Bootstrap = {
  account: Wire.BootstrapAccount;
  workspaces: Workspace[];
  activeWorkspaceId: string;
  subscriptions: Subscription[];
  articlePage: ArticlePage;
};
export type RuleDraft = {
  id?: string;
  subscriptionId: string;
  field: "title" | "full_text" | "title_or_full_text";
  phrase: string;
  action: "mark_read";
  enabled: boolean;
};
export type RulePreview = Wire.RulePreviewResponse;
export type RuleApplicationStatusName =
  | "queued"
  | "running"
  | "completed"
  | "cancelled"
  | "failed";
export type RuleApplicationAccepted = {
  operationId: string;
  status: RuleApplicationStatusName;
};
export type RuleApplicationStatus = RuleApplicationAccepted & {
  evaluated: number;
  cancelReason?: string;
};
export type FeedPreview = {
  title: string;
  kind: "rss" | "atom" | "json_feed" | "web_feed";
  url: string;
  articles: { title: string; publishedAt?: string }[];
};
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
export type WebFeedRecipeView = {
  subscriptionId: string;
  version: number;
  draft: WebFeedDraft;
};
export type VisualRect = Wire.VisualRect;
export type VisualCandidateGroup = {
  id: string;
  selector: SelectorDraft;
  count: number;
  boxes: VisualRect[];
};
export type VisualPreview = Wire.VisualPreviewResponse;
export type VisualSelection = {
  selector: SelectorDraft;
  count: number;
  similarItems: VisualRect[];
};
export type OpmlPreview = Wire.OpmlImportResponse;
export type SubscriptionActivity = {
  id: string;
  occurredAt: string;
  successful: boolean;
  durationMs?: number;
  discoveredItems?: number;
  diagnostic?: string;
};
export type PublicationHistory = {
  days: { date: string; count: number }[];
  undated: number;
  conflicting: number;
};
export type SourceUrlPreview = Wire.SourceUrlPreviewResponse;
export type SubscriptionExtraction = {
  sourceType: string;
  feedUrls: string[];
  recipeVersion?: number;
  recipeSummary?: string;
  lastPreview?: string;
};
export type SubscriptionRuleView = {
  id: string;
  scope: "subscription" | "workspace";
  summary: string;
  enabled: boolean;
};
export type SubscriptionDetail = Subscription & {
  feedUrls?: string[];
  extractionSummary?: string;
  lastPreview?: string;
  activity?: SubscriptionActivity[];
  rules?: SubscriptionRuleView[];
};
export type Transport = <T>(
  path: string,
  init: RequestInit | undefined,
  contract: ResponseContract,
) => Promise<T>;

export class ApiError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

export class ApiClient {
  readonly ai: AiClient;
  readonly glossary: GlossaryClient;
  constructor(private readonly transport: Transport) {
    this.ai = new AiClient(transport);
    this.glossary = new GlossaryClient(transport);
  }
  bootstrap = (position?: ArticlePagePosition) =>
    this.transport<Bootstrap>(
      `/api/bootstrap${articlePageQuery(position)}`,
      undefined,
      "BootstrapResponse",
    );
  signIn = (username: string, password: string) =>
    this.transport<void>(
      "/api/auth/sessions",
      json("POST", { username, password }),
      "SessionResponse",
    );
  signOut = () =>
    this.transport<void>("/api/auth/sessions", { method: "DELETE" }, "empty");
  acceptInvite = (token: string, username: string, password: string) =>
    this.transport<void>(
      "/api/auth/invites/accept",
      json("POST", { token, username, password }),
      "empty",
    );
  changePassword = (currentPassword: string, newPassword: string) =>
    this.transport<void>(
      "/api/auth/password/change",
      json("POST", {
        current_password: currentPassword,
        new_password: newPassword,
      }),
      "empty",
    );
  resetPassword = (token: string, newPassword: string) =>
    this.transport<void>(
      "/api/auth/password/reset",
      json("POST", { token, new_password: newPassword }),
      "empty",
    );
  listArticles = (
    workspaceId: string,
    view: string,
    subscriptionId?: string,
    cursor?: string,
    direction?: "older" | "newer",
  ) =>
    this.transport<ArticlePage>(
      `/api/articles?workspace_id=${enc(workspaceId)}&view=${enc(view)}${subscriptionId ? `&subscription_id=${enc(subscriptionId)}` : ""}${cursor ? `&cursor=${enc(cursor)}&direction=${direction ?? "older"}` : ""}`,
      undefined,
      "ArticlePageView",
    );
  getArticle = (workspaceId: string, articleId: string) =>
    this.transport<Article>(
      `/api/articles/${enc(articleId)}?workspace_id=${enc(workspaceId)}`,
      undefined,
      "ArticleView",
    );
  listSubscriptions = (workspaceId: string) =>
    this.transport<Subscription[]>(
      `/api/subscriptions?workspace_id=${enc(workspaceId)}`,
      undefined,
      "SubscriptionView[]",
    );
  getSubscription = (id: string) =>
    this.transport<SubscriptionDetail>(
      `/api/subscriptions/${enc(id)}`,
      undefined,
      "SubscriptionView",
    );
  saveSubscriptionNote = (id: string, note: string) =>
    this.transport<SubscriptionDetail>(
      `/api/subscriptions/${enc(id)}/note`,
      json("PUT", { note }),
      "SubscriptionView",
    );
  subscriptionActivity = (id: string) =>
    this.transport<SubscriptionActivity[]>(
      `/api/subscriptions/${enc(id)}/activity`,
      undefined,
      "SubscriptionActivityView[]",
    );
  publicationHistory = (id: string) =>
    this.transport<PublicationHistory>(
      `/api/subscriptions/${enc(id)}/publication-history`,
      undefined,
      "PublicationHistoryView",
    );
  subscriptionExtraction = (id: string) =>
    this.transport<SubscriptionExtraction>(
      `/api/subscriptions/${enc(id)}/extraction`,
      undefined,
      "SubscriptionExtractionView",
    );
  previewSubscriptionSourceUrl = (id: string, url: string) =>
    this.transport<SourceUrlPreview>(
      `/api/subscriptions/${enc(id)}/source-url/preview`,
      json("POST", { url }),
      "SourceUrlPreviewResponse",
    );
  commitSubscriptionSourceUrl = (id: string, previewToken: string) =>
    this.transport<Subscription>(
      `/api/subscriptions/${enc(id)}/source-url`,
      json("PUT", { previewToken }),
      "SubscriptionView",
    );
  updateArticle = (
    workspaceId: string,
    articleId: string,
    state: Partial<Pick<Article, "read" | "later">>,
  ) =>
    this.transport<Article>(
      `/api/articles/${enc(articleId)}/state?workspace_id=${enc(workspaceId)}`,
      json("POST", state),
      "ArticleView",
    );
  markAllRead = (workspaceId: string, view: string, subscriptionId?: string) =>
    this.transport<void>(
      `/api/workspaces/${enc(workspaceId)}/articles/mark-all-read`,
      json("POST", { view, subscription_id: subscriptionId }),
      "empty",
    );
  renameWorkspace = (id: string, name: string) =>
    this.transport<Workspace>(
      `/api/workspaces/${enc(id)}`,
      json("PATCH", { name }),
      "WorkspaceView",
    );
  createWorkspace = (name: string) =>
    this.transport<Workspace>(
      "/api/workspaces",
      json("POST", { name }),
      "WorkspaceView",
    );
  archiveWorkspace = (id: string, reason: string) =>
    this.transport<void>(
      `/api/workspaces/${enc(id)}/archive`,
      json("POST", { reason }),
      "empty",
    );
  restoreWorkspace = (id: string) =>
    this.transport<void>(
      `/api/workspaces/${enc(id)}/restore`,
      {
        method: "POST",
      },
      "empty",
    );
  discoverFeed = (url: string) =>
    this.transport<FeedPreview>(
      "/api/feeds/discover",
      json("POST", { url }),
      "FeedPreviewResponse",
    );
  addSubscription = (workspaceId: string, url: string, title?: string) =>
    this.transport<Subscription>(
      "/api/subscriptions",
      json("POST", { workspace_id: workspaceId, url, title }),
      "SubscriptionView",
    );
  refreshSubscription = (id: string) =>
    this.transport<void>(
      `/api/subscriptions/${enc(id)}/refresh`,
      {
        method: "POST",
      },
      "empty",
    );
  refreshFullText = (workspaceId: string, articleId: string) =>
    this.transport<void>(
      `/api/articles/${enc(articleId)}/full-text/refresh?workspace_id=${enc(workspaceId)}`,
      { method: "POST" },
      "empty",
    );
  pauseSubscription = (id: string, reason: string) =>
    this.transport<void>(
      `/api/subscriptions/${enc(id)}/pause`,
      json("POST", { reason }),
      "empty",
    );
  resumeSubscription = (id: string) =>
    this.transport<void>(
      `/api/subscriptions/${enc(id)}/resume`,
      {
        method: "POST",
      },
      "empty",
    );
  renameSubscription = (id: string, name: string) =>
    this.transport<Subscription>(
      `/api/subscriptions/${enc(id)}`,
      json("PATCH", { name }),
      "SubscriptionView",
    );
  unsubscribe = (id: string) =>
    this.transport<Subscription>(
      `/api/subscriptions/${enc(id)}`,
      {
        method: "DELETE",
      },
      "SubscriptionView",
    );
  restoreSubscription = (id: string) =>
    this.transport<Subscription>(
      `/api/subscriptions/${enc(id)}/restore`,
      {
        method: "POST",
      },
      "SubscriptionView",
    );
  listRules = (workspaceId: string) =>
    this.transport<RuleDraft[]>(
      `/api/rules?workspace_id=${enc(workspaceId)}`,
      undefined,
      "RuleDraft[]",
    );
  saveRule = (workspaceId: string, rule: RuleDraft) =>
    this.transport<RuleDraft>(
      `/api/rules${rule.id ? `/${enc(rule.id)}` : ""}?workspace_id=${enc(workspaceId)}`,
      json(rule.id ? "PUT" : "POST", rule),
      "RuleDraft",
    );
  previewRule = (workspaceId: string, rule: RuleDraft) =>
    this.transport<RulePreview>(
      `/api/rules/preview?workspace_id=${enc(workspaceId)}`,
      json("POST", rule),
      "RulePreviewResponse",
    );
  applyRule = (workspaceId: string, id: string) =>
    this.transport<RuleApplicationAccepted>(
      `/api/rules/${enc(id)}/apply?workspace_id=${enc(workspaceId)}`,
      { method: "POST" },
      "RuleApplicationAccepted",
    );
  ruleApplicationStatus = (workspaceId: string, operationId: string) =>
    this.transport<RuleApplicationStatus>(
      `/api/rule-applications/${enc(operationId)}?workspace_id=${enc(workspaceId)}`,
      undefined,
      "RuleApplicationStatus",
    );
  deleteRule = (workspaceId: string, id: string) =>
    this.transport<void>(
      `/api/rules/${enc(id)}?workspace_id=${enc(workspaceId)}`,
      { method: "DELETE" },
      "empty",
    );
  importOpml = (workspaceId: string, document: string, previewId?: string) =>
    this.transport<OpmlPreview>(
      "/api/opml/import",
      json("POST", {
        workspace_id: workspaceId,
        opml: document,
        apply: previewId !== undefined,
        preview_id: previewId,
      }),
      "OpmlImportResponse",
    );
  exportOpml = (workspaceId: string) =>
    this.transport<string>(
      `/api/opml/export?workspace_id=${enc(workspaceId)}`,
      undefined,
      "string",
    );
  previewWebFeed = (draft: WebFeedDraft) =>
    this.transport<FeedPreview>(
      "/api/web-feeds/recipes",
      json("POST", { ...draft, preview: true }),
      "FeedPreviewResponse",
    );
  createWebFeed = (draft: WebFeedDraft) =>
    this.transport<Subscription>(
      "/api/web-feeds/recipes",
      json("POST", draft),
      "SubscriptionView",
    );
  getWebFeedRecipe = (subscriptionId: string) =>
    this.transport<WebFeedRecipeView>(
      `/api/web-feeds/recipes/${enc(subscriptionId)}`,
      undefined,
      "WebFeedRecipeView",
    );
  updateWebFeedRecipe = (
    subscriptionId: string,
    expectedVersion: number,
    draft: WebFeedDraft,
  ) =>
    this.transport<WebFeedRecipeView>(
      `/api/web-feeds/recipes/${enc(subscriptionId)}`,
      json("PUT", { expectedVersion, draft }),
      "WebFeedRecipeView",
    );
  visualPreview = (
    workspaceId: string,
    url: string,
    viewport: WebFeedDraft["viewport"],
  ) =>
    this.transport<VisualPreview>(
      "/api/web-feeds/visual-previews",
      json("POST", { workspaceId, url, viewport }),
      "VisualPreviewResponse",
    );
  visualSelect = (
    workspaceId: string,
    snapshotToken: string,
    x: number,
    y: number,
  ) =>
    this.transport<VisualSelection>(
      "/api/web-feeds/visual-selections",
      json("POST", { workspaceId, snapshotToken, x, y }),
      "VisualSelectionResponse",
    );
  createInvite = (username: string) =>
    this.transport<{ url: string; expires_at: string }>(
      "/api/auth/invites",
      json("POST", { username }),
      "InviteResponse",
    );
  createPasswordReset = (username: string) =>
    this.transport<{ url: string; expires_at: string }>(
      "/api/auth/password-resets",
      json("POST", { username }),
      "PasswordResetResponse",
    );
}

export const fetchTransport: Transport = async <T>(
  path: string,
  init: RequestInit | undefined,
  contract: ResponseContract,
) => {
  const method = init?.method ?? "GET";
  const startedAt = performance.now();
  let status: number | "network-error" = "network-error";
  try {
    const response = await fetch(path, {
      credentials: "same-origin",
      headers: { Accept: "application/json", ...init?.headers },
      ...init,
    });
    status = response.status;
    if (response.status === 401)
      window.dispatchEvent(new CustomEvent("reader:unauthorized"));
    if (!response.ok) {
      let message = `Request failed (${response.status})`;
      try {
        const body = (await response.json()) as { message?: string };
        if (body.message) message = body.message;
      } catch {
        /* response may be empty */
      }
      throw new ApiError(response.status, message);
    }
    return decodeResponse(
      contract,
      response.status === 204 ? undefined : await response.json(),
    ) as T;
  } finally {
    reportApiRequest(path, method, startedAt, status);
  }
};
export const apiClient = new ApiClient(fetchTransport);
const enc = encodeURIComponent;
const articlePageQuery = (position?: ArticlePagePosition) => {
  if (!position) return "";
  const query = new URLSearchParams();
  if (position.view) query.set("view", position.view);
  if (position.subscriptionId)
    query.set("subscription_id", position.subscriptionId);
  if (position.cursor) {
    query.set("cursor", position.cursor);
    query.set("direction", position.direction ?? "older");
  }
  const value = query.toString();
  return value ? `?${value}` : "";
};
const json = (method: string, body: unknown): RequestInit => ({
  method,
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify(body),
});
