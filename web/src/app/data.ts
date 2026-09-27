import type {
  ArticleView,
  SubscriptionView,
  WorkspaceView,
} from "../api/generated";
/** UI drafts/fixtures may omit decoration fields. Core identity and state fields
 * use the generated wire contract; nullable server values stay nullable. */
type ViewModel<T, K extends keyof T> = Pick<T, K> & Partial<Omit<T, K>>;
export type Article = ViewModel<
  ArticleView,
  | "id"
  | "url"
  | "source"
  | "title"
  | "excerpt"
  | "body"
  | "age"
  | "read"
  | "later"
  | "fullText"
> & { originalUrl?: string };
export type Subscription = ViewModel<
  SubscriptionView,
  "id" | "name" | "count" | "status"
> & { pollingInterval?: string };
export type Workspace = WorkspaceView;
