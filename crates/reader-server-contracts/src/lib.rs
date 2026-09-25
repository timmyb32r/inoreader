use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReasonCommand {
    pub reason: String,
}
#[derive(Debug, Deserialize)]
pub struct MarkReadCommand {
    pub workspace_id: Uuid,
    pub read: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceView {
    pub id: Uuid,
    pub name: String,
    pub archived: bool,
    pub archive_reason: Option<String>,
    pub archive_reason_at: Option<DateTime<Utc>>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionView {
    pub id: Uuid,
    pub name: String,
    pub count: usize,
    pub status: String,
    pub last_update: Option<DateTime<Utc>>,
    pub incomplete: bool,
    pub continuation: Option<String>,
    pub error: Option<String>,
    pub editable_web_feed: bool,
    pub reason: Option<String>,
    pub reason_at: Option<DateTime<Utc>>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticleView {
    pub id: Uuid,
    pub url: String,
    pub source: String,
    pub sources: Vec<String>,
    pub subscription_ids: Vec<Uuid>,
    pub title: String,
    pub excerpt: String,
    pub body: Vec<String>,
    pub author: Option<String>,
    pub age: String,
    pub read: bool,
    pub saved: bool,
    pub later: bool,
    pub trash: bool,
    pub full_text: String,
    pub full_text_reason: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct WorkspaceResponse {
    pub workspace: WorkspaceView,
}
#[derive(Debug, Serialize)]
pub struct SubscriptionResponse {
    pub subscription: SubscriptionView,
}
#[derive(Debug, Serialize)]
pub struct ArticleResponse {
    pub article: ArticleView,
}
#[derive(Debug, Serialize)]
pub struct ArticleListResponse {
    pub articles: Vec<ArticleView>,
    pub generated_at: DateTime<Utc>,
}
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateInviteRequest {
    pub username: String,
}
#[derive(Debug, Serialize)]
pub struct InviteResponse {
    pub url: String,
    pub expires_at: DateTime<Utc>,
}
#[derive(Debug, Deserialize)]
pub struct AcceptInviteRequest {
    pub token: String,
    pub username: String,
    pub password: String,
}
#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub username: String,
    pub password: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub new_password: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatePasswordResetRequest {
    pub username: String,
}
#[derive(Debug, Serialize)]
pub struct PasswordResetResponse {
    pub url: String,
    pub expires_at: DateTime<Utc>,
}
#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub session_id: Uuid,
    pub expires_at: DateTime<Utc>,
}
#[derive(Debug, Deserialize)]
pub struct CreateWorkspaceRequest {
    pub name: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameWorkspaceRequest {
    pub name: String,
}
#[derive(Debug, Deserialize)]
pub struct AddSubscriptionRequest {
    pub workspace_id: Uuid,
    pub url: String,
    pub title: Option<String>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameSubscriptionRequest {
    pub name: String,
}
#[derive(Debug, Deserialize)]
pub struct ArticleListQuery {
    pub workspace_id: Uuid,
    pub view: Option<String>,
    pub subscription_id: Option<Uuid>,
    pub cursor: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct WorkspaceQuery {
    pub workspace_id: Uuid,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArticleStatePatch {
    pub read: Option<bool>,
    pub saved: Option<bool>,
    pub later: Option<bool>,
    pub trash: Option<bool>,
}
#[derive(Debug, Deserialize)]
pub struct TrashCommand {
    pub workspace_id: Uuid,
    pub trashed: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuleDraft {
    pub id: Option<Uuid>,
    pub subscription_id: Uuid,
    pub field: String,
    pub phrase: String,
    pub action: String,
    pub enabled: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RulePreviewResponse {
    pub matched_articles: usize,
    pub shared_articles: usize,
    pub total_subscription_articles: usize,
    pub sample_article_ids: Vec<Uuid>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleApplicationAccepted {
    pub operation_id: Uuid,
    pub status: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleApplicationStatus {
    pub operation_id: Uuid,
    pub status: String,
    pub evaluated: usize,
    pub cancel_reason: Option<String>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpmlImportRequest {
    pub workspace_id: Uuid,
    pub opml: String,
    pub apply: bool,
    pub preview_id: Option<Uuid>,
}
#[derive(Debug, Serialize)]
pub struct OpmlPreviewResponse {
    pub supported: usize,
    pub duplicates: usize,
    pub errors: Vec<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectorDraft {
    pub language: String,
    pub expression: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebFeedRecipeDraft {
    pub workspace_id: Uuid,
    pub url: String,
    pub selector: String,
    pub loading: String,
    pub preview: Option<bool>,
    #[serde(default)]
    pub selector_language: Option<String>,
    #[serde(default)]
    pub viewport: Option<String>,
    #[serde(default)]
    pub listing_url: Option<String>,
    #[serde(default)]
    pub card_selector: Option<String>,
    #[serde(default)]
    pub title_selector: Option<String>,
    #[serde(default)]
    pub date_selector: Option<String>,
    #[serde(default)]
    pub content_selector: Option<String>,
    #[serde(default)]
    pub wait_selector: Option<String>,
    #[serde(default)]
    pub url_pattern: Option<String>,
    #[serde(default)]
    pub max_pages: Option<usize>,
    #[serde(default)]
    pub hide_overlays: Vec<SelectorDraft>,
    #[serde(default)]
    pub start_pages: Vec<String>,
    #[serde(default)]
    pub next_page: Option<SelectorDraft>,
    #[serde(default)]
    pub load_more: Option<SelectorDraft>,
    #[serde(default)]
    pub load_more_clicks: usize,
    #[serde(default)]
    pub scrolls: usize,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebFeedRecipeView {
    pub subscription_id: Uuid,
    pub version: u64,
    pub draft: WebFeedRecipeDraft,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateWebFeedRecipeRequest {
    pub expected_version: u64,
    pub draft: WebFeedRecipeDraft,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VisualPreviewRequest {
    pub workspace_id: Uuid,
    pub url: String,
    pub viewport: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VisualSelectionRequest {
    pub workspace_id: Uuid,
    pub snapshot_token: Uuid,
    pub x: f64,
    pub y: f64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualCandidateGroup {
    pub id: String,
    pub selector: SelectorDraft,
    pub count: usize,
    pub boxes: Vec<VisualRect>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualPreviewResponse {
    pub snapshot_token: Uuid,
    pub expires_at: DateTime<Utc>,
    pub image_data_url: String,
    pub width: u32,
    pub height: u32,
    pub groups: Vec<VisualCandidateGroup>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualSelectionResponse {
    pub selector: SelectorDraft,
    pub count: usize,
    pub similar_items: Vec<VisualRect>,
}
#[derive(Debug, Serialize)]
pub struct OperationAccepted {
    pub operation_id: Uuid,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarkAllReadRequest {
    pub view: String,
    pub subscription_id: Option<Uuid>,
}
#[derive(Debug, Deserialize)]
pub struct RuleListQuery {
    pub workspace_id: Uuid,
}
#[derive(Debug, Deserialize)]
pub struct OpmlExportQuery {
    pub workspace_id: Uuid,
}
#[derive(Debug, Serialize)]
pub struct OpmlImportResponse {
    pub preview_id: Uuid,
    pub subscriptions: usize,
    pub warnings: Vec<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapAccount {
    pub display_name: String,
    pub initials: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapResponse {
    pub account: BootstrapAccount,
    pub workspaces: Vec<WorkspaceView>,
    pub active_workspace_id: Uuid,
    pub subscriptions: Vec<SubscriptionView>,
    pub articles: Vec<ArticleView>,
    pub new_article_count: usize,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedDiscoverRequest {
    pub url: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedPreviewArticle {
    pub title: String,
    pub published_at: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedPreviewResponse {
    pub title: String,
    pub kind: String,
    pub url: String,
    pub available_items: usize,
    pub initial_items: usize,
    pub incomplete: bool,
    pub articles: Vec<FeedPreviewArticle>,
}
