import type { Article, Subscription, Workspace } from "../api/viewModels";
import { reportApiRequest } from "../performanceDiagnostics";
import { WikiClient } from "./wiki";
import { ZhihuClient } from "./zhihu";
import { AiClient } from "./ai";
import {
  decodeResponse,
  type ResponseContract,
  type ResponseValue,
} from "./decode";
import type * as Wire from "./generated";
import { GlossaryClient } from "./glossary";

export type ArticlePage = Pick<
  Wire.ArticlePageView,
  "total" | "unreadTotal"
> & {
  articles: Article[];
  newerCursor?: string | null;
  olderCursor?: string | null;
};
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
export type RuleDraft = Wire.RuleDraft;
export type RulePreview = Wire.RulePreviewResponse;
export type RuleApplicationStatusName = Wire.RuleApplicationStatus["status"];
export type RuleApplicationAccepted = Wire.RuleApplicationAccepted;
export type RuleApplicationStatus = Wire.RuleApplicationStatus;
export type FeedPreview = {
  title: string;
  kind: "rss" | "atom" | "json_feed" | "web_feed";
  url: string;
  articles: { title: string; publishedAt?: string }[];
};
export type SelectorDraft = Wire.SelectorDraft;
export type WebFeedDraft = Wire.WebFeedRecipeDraft;
export type WebFeedRecipeView = Wire.WebFeedRecipeView;
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
export type SubscriptionActivity = Wire.SubscriptionActivityView;
export type PublicationHistory = {
  days: { date: string; count: number }[];
  undated: number;
  conflicting: number;
};
export type SourceUrlPreview = Wire.SourceUrlPreviewResponse;
export type SubscriptionExtraction = Wire.SubscriptionExtractionView;
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
export type Transport = <C extends ResponseContract>(
  path: string,
  init: RequestInit | undefined,
  contract: C,
) => Promise<ResponseValue<C>>;

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
  readonly zhihu: ZhihuClient;
  readonly wiki: WikiClient;
  constructor(private readonly transport: Transport) {
    this.ai = new AiClient(transport);
    this.glossary = new GlossaryClient(transport);
    this.zhihu = new ZhihuClient(transport);
    this.wiki = new WikiClient(transport);
  }
  bootstrap = (position?: ArticlePagePosition): Promise<Bootstrap> =>
    this.transport(
      `/api/bootstrap${articlePageQuery(position)}`,
      undefined,
      "BootstrapResponse",
    );
  signIn = (username: string, password: string) =>
    this.transport(
      "/api/auth/sessions",
      json("POST", { username, password }),
      "SessionResponse",
    );
  signOut = () =>
    this.transport("/api/auth/sessions", { method: "DELETE" }, "empty");
  acceptInvite = (token: string, username: string, password: string) =>
    this.transport(
      "/api/auth/invites/accept",
      json("POST", { token, username, password }),
      "empty",
    );
  changePassword = (currentPassword: string, newPassword: string) =>
    this.transport(
      "/api/auth/password/change",
      json("POST", {
        current_password: currentPassword,
        new_password: newPassword,
      }),
      "empty",
    );
  resetPassword = (token: string, newPassword: string) =>
    this.transport(
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
  ): Promise<ArticlePage> =>
    this.transport(
      `/api/articles?workspace_id=${enc(workspaceId)}&view=${enc(view)}${subscriptionId ? `&subscription_id=${enc(subscriptionId)}` : ""}${cursor ? `&cursor=${enc(cursor)}&direction=${direction ?? "older"}` : ""}`,
      undefined,
      "ArticlePageView",
    );
  getArticle = (workspaceId: string, articleId: string): Promise<Article> =>
    this.transport(
      `/api/articles/${enc(articleId)}?workspace_id=${enc(workspaceId)}`,
      undefined,
      "ArticleView",
    );
  listSubscriptions = (workspaceId: string): Promise<Subscription[]> =>
    this.transport(
      `/api/subscriptions?workspace_id=${enc(workspaceId)}`,
      undefined,
      "SubscriptionView[]",
    );
  getSubscription = (id: string): Promise<SubscriptionDetail> =>
    this.transport(
      `/api/subscriptions/${enc(id)}`,
      undefined,
      "SubscriptionView",
    );
  saveSubscriptionNote = (id: string, note: string) =>
    this.transport(
      `/api/subscriptions/${enc(id)}/note`,
      json("PUT", { note }),
      "SubscriptionView",
    );
  subscriptionActivity = (id: string) =>
    this.transport(
      `/api/subscriptions/${enc(id)}/activity`,
      undefined,
      "SubscriptionActivityView[]",
    );
  publicationHistory = (id: string) =>
    this.transport(
      `/api/subscriptions/${enc(id)}/publication-history`,
      undefined,
      "PublicationHistoryView",
    );
  subscriptionExtraction = (id: string) =>
    this.transport(
      `/api/subscriptions/${enc(id)}/extraction`,
      undefined,
      "SubscriptionExtractionView",
    );
  previewSubscriptionSourceUrl = (id: string, url: string) =>
    this.transport(
      `/api/subscriptions/${enc(id)}/source-url/preview`,
      json("POST", { url }),
      "SourceUrlPreviewResponse",
    );
  commitSubscriptionSourceUrl = (id: string, previewToken: string) =>
    this.transport(
      `/api/subscriptions/${enc(id)}/source-url`,
      json("PUT", { previewToken }),
      "SubscriptionView",
    );
  updateArticle = (
    workspaceId: string,
    articleId: string,
    state: Partial<Pick<Article, "read" | "later">>,
  ) =>
    this.transport(
      `/api/articles/${enc(articleId)}/state?workspace_id=${enc(workspaceId)}`,
      json("POST", state),
      "ArticleView",
    );
  markAllRead = (workspaceId: string, view: string, subscriptionId?: string) =>
    this.transport(
      `/api/workspaces/${enc(workspaceId)}/articles/mark-all-read`,
      json("POST", { view, subscription_id: subscriptionId }),
      "empty",
    );
  renameWorkspace = (id: string, name: string) =>
    this.transport(
      `/api/workspaces/${enc(id)}`,
      json("PATCH", { name }),
      "WorkspaceView",
    );
  createWorkspace = (name: string) =>
    this.transport("/api/workspaces", json("POST", { name }), "WorkspaceView");
  archiveWorkspace = (id: string, reason: string) =>
    this.transport(
      `/api/workspaces/${enc(id)}/archive`,
      json("POST", { reason }),
      "empty",
    );
  restoreWorkspace = (id: string) =>
    this.transport(
      `/api/workspaces/${enc(id)}/restore`,
      {
        method: "POST",
      },
      "empty",
    );
  discoverFeed = (url: string) =>
    this.transport(
      "/api/feeds/discover",
      json("POST", { url }),
      "FeedPreviewResponse",
    );
  addSubscription = (workspaceId: string, url: string, title?: string) =>
    this.transport(
      "/api/subscriptions",
      json("POST", { workspace_id: workspaceId, url, title }),
      "SubscriptionView",
    );
  refreshSubscription = (id: string) =>
    this.transport(
      `/api/subscriptions/${enc(id)}/refresh`,
      {
        method: "POST",
      },
      "empty",
    );
  refreshFullText = (workspaceId: string, articleId: string) =>
    this.transport(
      `/api/articles/${enc(articleId)}/full-text/refresh?workspace_id=${enc(workspaceId)}`,
      { method: "POST" },
      "empty",
    );
  pauseSubscription = (id: string, reason: string) =>
    this.transport(
      `/api/subscriptions/${enc(id)}/pause`,
      json("POST", { reason }),
      "empty",
    );
  resumeSubscription = (id: string) =>
    this.transport(
      `/api/subscriptions/${enc(id)}/resume`,
      {
        method: "POST",
      },
      "empty",
    );
  renameSubscription = (id: string, name: string) =>
    this.transport(
      `/api/subscriptions/${enc(id)}`,
      json("PATCH", { name }),
      "SubscriptionView",
    );
  archiveSubscription = (id: string) =>
    this.transport(
      `/api/subscriptions/${enc(id)}/archive`,
      { method: "POST" },
      "SubscriptionView",
    );
  deleteSubscription = (id: string) =>
    this.transport(
      `/api/subscriptions/${enc(id)}/delete`,
      { method: "POST" },
      "empty",
    );
  restoreSubscription = (id: string) =>
    this.transport(
      `/api/subscriptions/${enc(id)}/restore`,
      {
        method: "POST",
      },
      "SubscriptionView",
    );
  listRules = (workspaceId: string) =>
    this.transport(
      `/api/rules?workspace_id=${enc(workspaceId)}`,
      undefined,
      "RuleDraft[]",
    );
  saveRule = (workspaceId: string, rule: RuleDraft) =>
    this.transport(
      `/api/rules${rule.id ? `/${enc(rule.id)}` : ""}?workspace_id=${enc(workspaceId)}`,
      json(rule.id ? "PUT" : "POST", rule),
      "RuleDraft",
    );
  previewRule = (workspaceId: string, rule: RuleDraft) =>
    this.transport(
      `/api/rules/preview?workspace_id=${enc(workspaceId)}`,
      json("POST", rule),
      "RulePreviewResponse",
    );
  applyRule = (workspaceId: string, id: string) =>
    this.transport(
      `/api/rules/${enc(id)}/apply?workspace_id=${enc(workspaceId)}`,
      { method: "POST" },
      "RuleApplicationAccepted",
    );
  ruleApplicationStatus = (workspaceId: string, operationId: string) =>
    this.transport(
      `/api/rule-applications/${enc(operationId)}?workspace_id=${enc(workspaceId)}`,
      undefined,
      "RuleApplicationStatus",
    );
  deleteRule = (workspaceId: string, id: string) =>
    this.transport(
      `/api/rules/${enc(id)}?workspace_id=${enc(workspaceId)}`,
      { method: "DELETE" },
      "empty",
    );
  importOpml = (workspaceId: string, document: string, previewId?: string) =>
    this.transport(
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
    this.transport(
      `/api/opml/export?workspace_id=${enc(workspaceId)}`,
      undefined,
      "string",
    );
  previewWebFeed = (draft: WebFeedDraft) =>
    this.transport(
      "/api/web-feeds/recipes",
      json("POST", { ...draft, preview: true }),
      "FeedPreviewResponse",
    );
  createWebFeed = (draft: WebFeedDraft) =>
    this.transport(
      "/api/web-feeds/recipes",
      json("POST", draft),
      "SubscriptionView",
    );
  getWebFeedRecipe = (subscriptionId: string) =>
    this.transport(
      `/api/web-feeds/recipes/${enc(subscriptionId)}`,
      undefined,
      "WebFeedRecipeView",
    );
  updateWebFeedRecipe = (
    subscriptionId: string,
    expectedVersion: number,
    draft: WebFeedDraft,
  ) =>
    this.transport(
      `/api/web-feeds/recipes/${enc(subscriptionId)}`,
      json("PUT", { expectedVersion, draft }),
      "WebFeedRecipeView",
    );
  visualPreview = (
    workspaceId: string,
    url: string,
    viewport: WebFeedDraft["viewport"],
  ) =>
    this.transport(
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
    this.transport(
      "/api/web-feeds/visual-selections",
      json("POST", { workspaceId, snapshotToken, x, y }),
      "VisualSelectionResponse",
    );
  createInvite = (username: string) =>
    this.transport(
      "/api/auth/invites",
      json("POST", { username }),
      "InviteResponse",
    );
  createPasswordReset = (username: string) =>
    this.transport(
      "/api/auth/password-resets",
      json("POST", { username }),
      "PasswordResetResponse",
    );
}

export const fetchTransport: Transport = async <C extends ResponseContract>(
  path: string,
  init: RequestInit | undefined,
  contract: C,
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
    );
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
