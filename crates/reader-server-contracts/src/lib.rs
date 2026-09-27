use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(schemars::JsonSchema, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReasonCommand {
    pub reason: String,
}
#[derive(schemars::JsonSchema, Debug, Deserialize)]
pub struct MarkReadCommand {
    pub workspace_id: Uuid,
    pub read: bool,
}
#[derive(schemars::JsonSchema, Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceView {
    pub id: Uuid,
    pub name: String,
    pub archived: bool,
    pub archive_reason: Option<String>,
    pub archive_reason_at: Option<DateTime<Utc>>,
}
#[derive(schemars::JsonSchema, Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionView {
    pub id: Uuid,
    pub name: String,
    pub source_title: String,
    pub custom_name: Option<String>,
    pub personal_note: String,
    pub source_url: String,
    pub icon_data_url: Option<String>,
    pub source_type: SourceTypeView,
    pub created_at: Option<DateTime<Utc>>,
    pub count: usize,
    pub unread_count: usize,
    pub status: SubscriptionStatusView,
    pub last_update: Option<DateTime<Utc>>,
    pub last_error_at: Option<DateTime<Utc>>,
    pub consecutive_failures: u32,
    pub needs_attention: bool,
    pub attention_reason: Option<String>,
    pub incomplete: bool,
    pub continuation: Option<String>,
    pub error: Option<String>,
    pub editable_web_feed: bool,
    pub reason: Option<String>,
    pub reason_at: Option<DateTime<Utc>>,
}
#[derive(schemars::JsonSchema, Clone, Debug, Serialize)]
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
    pub body_html: Option<String>,
    pub author: Option<String>,
    pub age: String,
    pub read: bool,
    pub later: bool,
    pub full_text: FullTextStatus,
    pub full_text_reason: Option<String>,
}
#[derive(schemars::JsonSchema, Debug, Serialize)]
pub struct WorkspaceResponse {
    pub workspace: WorkspaceView,
}
#[derive(schemars::JsonSchema, Debug, Serialize)]
pub struct SubscriptionResponse {
    pub subscription: SubscriptionView,
}
#[derive(schemars::JsonSchema, Debug, Serialize)]
pub struct ArticleResponse {
    pub article: ArticleView,
}
#[derive(schemars::JsonSchema, Debug, Serialize)]
pub struct ArticleListResponse {
    pub articles: Vec<ArticleView>,
    pub generated_at: DateTime<Utc>,
}
#[derive(schemars::JsonSchema, Debug, Serialize)]
pub struct ApiError {
    pub code: &'static str,
    pub message: String,
}

#[derive(schemars::JsonSchema, Debug, Deserialize)]
pub struct CreateInviteRequest {
    pub username: String,
}
#[derive(schemars::JsonSchema, Debug, Serialize)]
pub struct InviteResponse {
    pub url: String,
    pub expires_at: DateTime<Utc>,
}
#[derive(schemars::JsonSchema, Debug, Deserialize)]
pub struct AcceptInviteRequest {
    pub token: String,
    pub username: String,
    pub password: String,
}
#[derive(schemars::JsonSchema, Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub username: String,
    pub password: String,
}
#[derive(schemars::JsonSchema, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}
#[derive(schemars::JsonSchema, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub new_password: String,
}
#[derive(schemars::JsonSchema, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatePasswordResetRequest {
    pub username: String,
}
#[derive(schemars::JsonSchema, Debug, Serialize)]
pub struct PasswordResetResponse {
    pub url: String,
    pub expires_at: DateTime<Utc>,
}
#[derive(schemars::JsonSchema, Debug, Serialize)]
pub struct SessionResponse {
    pub session_id: Uuid,
    pub expires_at: DateTime<Utc>,
}
#[derive(schemars::JsonSchema, Debug, Deserialize)]
pub struct CreateWorkspaceRequest {
    pub name: String,
}
#[derive(schemars::JsonSchema, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameWorkspaceRequest {
    pub name: String,
}
#[derive(schemars::JsonSchema, Debug, Deserialize)]
pub struct AddSubscriptionRequest {
    pub workspace_id: Uuid,
    pub url: String,
    pub title: Option<String>,
}
#[derive(schemars::JsonSchema, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameSubscriptionRequest {
    pub name: String,
}
#[derive(schemars::JsonSchema, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveSubscriptionNoteRequest {
    pub note: String,
}
#[derive(schemars::JsonSchema, Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionActivityView {
    pub id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub successful: bool,
    pub duration_ms: Option<u64>,
    pub discovered_items: Option<usize>,
    pub diagnostic: Option<String>,
}
#[derive(schemars::JsonSchema, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewSourceUrlRequest {
    pub url: String,
}
#[derive(schemars::JsonSchema, Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceUrlPreviewResponse {
    pub token: Uuid,
    pub url: String,
    pub title: String,
    pub expires_at: DateTime<Utc>,
}
#[derive(schemars::JsonSchema, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommitSourceUrlRequest {
    pub preview_token: Uuid,
}
#[derive(schemars::JsonSchema, Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionExtractionView {
    pub source_type: SourceTypeView,
    pub feed_urls: Vec<String>,
    pub recipe_version: Option<u64>,
    pub recipe_summary: Option<String>,
    pub last_preview: Option<DateTime<Utc>>,
}
#[derive(schemars::JsonSchema, Debug, Deserialize)]
pub struct ArticleListQuery {
    pub workspace_id: Uuid,
    pub view: Option<String>,
    pub subscription_id: Option<Uuid>,
    pub cursor: Option<String>,
    pub direction: Option<String>,
}
#[derive(schemars::JsonSchema, Debug, Default, Deserialize)]
pub struct BootstrapQuery {
    pub view: Option<String>,
    pub subscription_id: Option<Uuid>,
    pub cursor: Option<String>,
    pub direction: Option<String>,
}
#[derive(schemars::JsonSchema, Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticlePageView {
    pub articles: Vec<ArticleView>,
    pub total: usize,
    pub unread_total: usize,
    pub newer_cursor: Option<String>,
    pub older_cursor: Option<String>,
}
#[derive(schemars::JsonSchema, Debug, Deserialize)]
pub struct WorkspaceQuery {
    pub workspace_id: Uuid,
}
#[derive(schemars::JsonSchema, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArticleStatePatch {
    pub read: Option<bool>,
    pub later: Option<bool>,
}
#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuleDraft {
    pub id: Option<Uuid>,
    pub subscription_id: Uuid,
    pub field: String,
    pub phrase: String,
    pub action: String,
    pub enabled: bool,
}
#[derive(schemars::JsonSchema, Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RulePreviewResponse {
    pub matched_articles: usize,
    pub shared_articles: usize,
    pub total_subscription_articles: usize,
    pub sample_article_ids: Vec<Uuid>,
}
#[derive(schemars::JsonSchema, Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleApplicationAccepted {
    pub operation_id: Uuid,
    pub status: String,
}
#[derive(schemars::JsonSchema, Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleApplicationStatus {
    pub operation_id: Uuid,
    pub status: String,
    pub evaluated: usize,
    pub cancel_reason: Option<String>,
}
#[derive(schemars::JsonSchema, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpmlImportRequest {
    pub workspace_id: Uuid,
    pub opml: String,
    pub apply: bool,
    pub preview_id: Option<Uuid>,
}
#[derive(schemars::JsonSchema, Debug, Serialize)]
pub struct OpmlPreviewResponse {
    pub supported: usize,
    pub duplicates: usize,
    pub errors: Vec<String>,
}
pub use reader_core::{SelectorDraft, WebFeedRecipeDraft};
#[derive(schemars::JsonSchema, Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebFeedRecipeView {
    pub subscription_id: Uuid,
    pub version: u64,
    pub draft: WebFeedRecipeDraft,
}
#[derive(schemars::JsonSchema, Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateWebFeedRecipeRequest {
    pub expected_version: u64,
    pub draft: WebFeedRecipeDraft,
}
#[derive(schemars::JsonSchema, Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VisualPreviewRequest {
    pub workspace_id: Uuid,
    pub url: String,
    pub viewport: String,
}
#[derive(schemars::JsonSchema, Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VisualSelectionRequest {
    pub workspace_id: Uuid,
    pub snapshot_token: Uuid,
    pub x: f64,
    pub y: f64,
}
#[derive(schemars::JsonSchema, Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(schemars::JsonSchema, Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualCandidateGroup {
    pub id: String,
    pub selector: SelectorDraft,
    pub count: usize,
    pub boxes: Vec<VisualRect>,
}
#[derive(schemars::JsonSchema, Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualPreviewResponse {
    pub snapshot_token: Uuid,
    pub expires_at: DateTime<Utc>,
    pub image_data_url: String,
    pub width: u32,
    pub height: u32,
    pub groups: Vec<VisualCandidateGroup>,
}
#[derive(schemars::JsonSchema, Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualSelectionResponse {
    pub selector: SelectorDraft,
    pub count: usize,
    pub similar_items: Vec<VisualRect>,
}
#[derive(schemars::JsonSchema, Debug, Serialize)]
pub struct OperationAccepted {
    pub operation_id: Uuid,
}
#[derive(schemars::JsonSchema, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarkAllReadRequest {
    pub view: String,
    pub subscription_id: Option<Uuid>,
}
#[derive(schemars::JsonSchema, Debug, Deserialize)]
pub struct RuleListQuery {
    pub workspace_id: Uuid,
}
#[derive(schemars::JsonSchema, Debug, Deserialize)]
pub struct OpmlExportQuery {
    pub workspace_id: Uuid,
}
#[derive(schemars::JsonSchema, Debug, Serialize)]
pub struct OpmlImportResponse {
    pub preview_id: Uuid,
    pub subscriptions: usize,
    pub warnings: Vec<String>,
}
#[derive(schemars::JsonSchema, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapAccount {
    pub id: Uuid,
    pub display_name: String,
    pub initials: String,
}
#[derive(schemars::JsonSchema, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapResponse {
    pub account: BootstrapAccount,
    pub workspaces: Vec<WorkspaceView>,
    pub active_workspace_id: Uuid,
    pub subscriptions: Vec<SubscriptionView>,
    pub article_page: ArticlePageView,
}
#[derive(schemars::JsonSchema, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedDiscoverRequest {
    pub url: String,
}
#[derive(schemars::JsonSchema, Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedPreviewArticle {
    pub title: String,
    pub published_at: Option<String>,
}
#[derive(schemars::JsonSchema, Clone, Debug, Serialize)]
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

#[derive(Clone, Copy, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FullTextStatus {
    Ready,
    Pending,
    Failed,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatusView {
    Active,
    Paused,
    Archived,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceTypeView {
    Feed,
    Web,
    BuiltIn,
}

#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicationHistoryView {
    pub days: Vec<PublicationDayView>,
    pub undated: u64,
    pub conflicting: u64,
}
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct PublicationDayView {
    pub date: String,
    pub count: u64,
}
