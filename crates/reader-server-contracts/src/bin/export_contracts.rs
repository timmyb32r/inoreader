use reader_server_contracts::*;
fn main() {
    let mut schemas = serde_json::Map::new();
    schemas.insert(
        "PublicationHistoryView".into(),
        serde_json::to_value(schemars::schema_for!(PublicationHistoryView)).unwrap(),
    );
    schemas.insert(
        "ReasonCommand".into(),
        serde_json::to_value(schemars::schema_for!(ReasonCommand)).expect("serializable schema"),
    );
    schemas.insert(
        "MarkReadCommand".into(),
        serde_json::to_value(schemars::schema_for!(MarkReadCommand)).expect("serializable schema"),
    );
    schemas.insert(
        "WorkspaceView".into(),
        serde_json::to_value(schemars::schema_for!(WorkspaceView)).expect("serializable schema"),
    );
    schemas.insert(
        "SubscriptionView".into(),
        serde_json::to_value(schemars::schema_for!(SubscriptionView)).expect("serializable schema"),
    );
    schemas.insert(
        "ArticleView".into(),
        serde_json::to_value(schemars::schema_for!(ArticleView)).expect("serializable schema"),
    );
    schemas.insert(
        "WorkspaceResponse".into(),
        serde_json::to_value(schemars::schema_for!(WorkspaceResponse))
            .expect("serializable schema"),
    );
    schemas.insert(
        "SubscriptionResponse".into(),
        serde_json::to_value(schemars::schema_for!(SubscriptionResponse))
            .expect("serializable schema"),
    );
    schemas.insert(
        "ArticleResponse".into(),
        serde_json::to_value(schemars::schema_for!(ArticleResponse)).expect("serializable schema"),
    );
    schemas.insert(
        "ArticleListResponse".into(),
        serde_json::to_value(schemars::schema_for!(ArticleListResponse))
            .expect("serializable schema"),
    );
    schemas.insert(
        "ApiError".into(),
        serde_json::to_value(schemars::schema_for!(ApiError)).expect("serializable schema"),
    );
    schemas.insert(
        "CreateInviteRequest".into(),
        serde_json::to_value(schemars::schema_for!(CreateInviteRequest))
            .expect("serializable schema"),
    );
    schemas.insert(
        "InviteResponse".into(),
        serde_json::to_value(schemars::schema_for!(InviteResponse)).expect("serializable schema"),
    );
    schemas.insert(
        "AcceptInviteRequest".into(),
        serde_json::to_value(schemars::schema_for!(AcceptInviteRequest))
            .expect("serializable schema"),
    );
    schemas.insert(
        "CreateSessionRequest".into(),
        serde_json::to_value(schemars::schema_for!(CreateSessionRequest))
            .expect("serializable schema"),
    );
    schemas.insert(
        "ChangePasswordRequest".into(),
        serde_json::to_value(schemars::schema_for!(ChangePasswordRequest))
            .expect("serializable schema"),
    );
    schemas.insert(
        "ResetPasswordRequest".into(),
        serde_json::to_value(schemars::schema_for!(ResetPasswordRequest))
            .expect("serializable schema"),
    );
    schemas.insert(
        "CreatePasswordResetRequest".into(),
        serde_json::to_value(schemars::schema_for!(CreatePasswordResetRequest))
            .expect("serializable schema"),
    );
    schemas.insert(
        "PasswordResetResponse".into(),
        serde_json::to_value(schemars::schema_for!(PasswordResetResponse))
            .expect("serializable schema"),
    );
    schemas.insert(
        "SessionResponse".into(),
        serde_json::to_value(schemars::schema_for!(SessionResponse)).expect("serializable schema"),
    );
    schemas.insert(
        "CreateWorkspaceRequest".into(),
        serde_json::to_value(schemars::schema_for!(CreateWorkspaceRequest))
            .expect("serializable schema"),
    );
    schemas.insert(
        "RenameWorkspaceRequest".into(),
        serde_json::to_value(schemars::schema_for!(RenameWorkspaceRequest))
            .expect("serializable schema"),
    );
    schemas.insert(
        "AddSubscriptionRequest".into(),
        serde_json::to_value(schemars::schema_for!(AddSubscriptionRequest))
            .expect("serializable schema"),
    );
    schemas.insert(
        "RenameSubscriptionRequest".into(),
        serde_json::to_value(schemars::schema_for!(RenameSubscriptionRequest))
            .expect("serializable schema"),
    );
    schemas.insert(
        "SaveSubscriptionNoteRequest".into(),
        serde_json::to_value(schemars::schema_for!(SaveSubscriptionNoteRequest))
            .expect("serializable schema"),
    );
    schemas.insert(
        "SubscriptionActivityView".into(),
        serde_json::to_value(schemars::schema_for!(SubscriptionActivityView))
            .expect("serializable schema"),
    );
    schemas.insert(
        "PreviewSourceUrlRequest".into(),
        serde_json::to_value(schemars::schema_for!(PreviewSourceUrlRequest))
            .expect("serializable schema"),
    );
    schemas.insert(
        "SourceUrlPreviewResponse".into(),
        serde_json::to_value(schemars::schema_for!(SourceUrlPreviewResponse))
            .expect("serializable schema"),
    );
    schemas.insert(
        "CommitSourceUrlRequest".into(),
        serde_json::to_value(schemars::schema_for!(CommitSourceUrlRequest))
            .expect("serializable schema"),
    );
    schemas.insert(
        "SubscriptionExtractionView".into(),
        serde_json::to_value(schemars::schema_for!(SubscriptionExtractionView))
            .expect("serializable schema"),
    );
    schemas.insert(
        "ArticleListQuery".into(),
        serde_json::to_value(schemars::schema_for!(ArticleListQuery)).expect("serializable schema"),
    );
    schemas.insert(
        "BootstrapQuery".into(),
        serde_json::to_value(schemars::schema_for!(BootstrapQuery)).expect("serializable schema"),
    );
    schemas.insert(
        "ArticlePageView".into(),
        serde_json::to_value(schemars::schema_for!(ArticlePageView)).expect("serializable schema"),
    );
    schemas.insert(
        "WorkspaceQuery".into(),
        serde_json::to_value(schemars::schema_for!(WorkspaceQuery)).expect("serializable schema"),
    );
    schemas.insert(
        "ArticleStatePatch".into(),
        serde_json::to_value(schemars::schema_for!(ArticleStatePatch))
            .expect("serializable schema"),
    );
    schemas.insert(
        "RuleDraft".into(),
        serde_json::to_value(schemars::schema_for!(RuleDraft)).expect("serializable schema"),
    );
    schemas.insert(
        "RulePreviewResponse".into(),
        serde_json::to_value(schemars::schema_for!(RulePreviewResponse))
            .expect("serializable schema"),
    );
    schemas.insert(
        "RuleApplicationAccepted".into(),
        serde_json::to_value(schemars::schema_for!(RuleApplicationAccepted))
            .expect("serializable schema"),
    );
    schemas.insert(
        "RuleApplicationStatus".into(),
        serde_json::to_value(schemars::schema_for!(RuleApplicationStatus))
            .expect("serializable schema"),
    );
    schemas.insert(
        "OpmlImportRequest".into(),
        serde_json::to_value(schemars::schema_for!(OpmlImportRequest))
            .expect("serializable schema"),
    );
    schemas.insert(
        "OpmlPreviewResponse".into(),
        serde_json::to_value(schemars::schema_for!(OpmlPreviewResponse))
            .expect("serializable schema"),
    );
    schemas.insert(
        "SelectorDraft".into(),
        serde_json::to_value(schemars::schema_for!(SelectorDraft)).expect("serializable schema"),
    );
    schemas.insert(
        "WebFeedRecipeDraft".into(),
        serde_json::to_value(schemars::schema_for!(WebFeedRecipeDraft))
            .expect("serializable schema"),
    );
    schemas.insert(
        "WebFeedRecipeView".into(),
        serde_json::to_value(schemars::schema_for!(WebFeedRecipeView))
            .expect("serializable schema"),
    );
    schemas.insert(
        "UpdateWebFeedRecipeRequest".into(),
        serde_json::to_value(schemars::schema_for!(UpdateWebFeedRecipeRequest))
            .expect("serializable schema"),
    );
    schemas.insert(
        "VisualPreviewRequest".into(),
        serde_json::to_value(schemars::schema_for!(VisualPreviewRequest))
            .expect("serializable schema"),
    );
    schemas.insert(
        "VisualSelectionRequest".into(),
        serde_json::to_value(schemars::schema_for!(VisualSelectionRequest))
            .expect("serializable schema"),
    );
    schemas.insert(
        "VisualRect".into(),
        serde_json::to_value(schemars::schema_for!(VisualRect)).expect("serializable schema"),
    );
    schemas.insert(
        "VisualCandidateGroup".into(),
        serde_json::to_value(schemars::schema_for!(VisualCandidateGroup))
            .expect("serializable schema"),
    );
    schemas.insert(
        "VisualPreviewResponse".into(),
        serde_json::to_value(schemars::schema_for!(VisualPreviewResponse))
            .expect("serializable schema"),
    );
    schemas.insert(
        "VisualSelectionResponse".into(),
        serde_json::to_value(schemars::schema_for!(VisualSelectionResponse))
            .expect("serializable schema"),
    );
    schemas.insert(
        "OperationAccepted".into(),
        serde_json::to_value(schemars::schema_for!(OperationAccepted))
            .expect("serializable schema"),
    );
    schemas.insert(
        "MarkAllReadRequest".into(),
        serde_json::to_value(schemars::schema_for!(MarkAllReadRequest))
            .expect("serializable schema"),
    );
    schemas.insert(
        "RuleListQuery".into(),
        serde_json::to_value(schemars::schema_for!(RuleListQuery)).expect("serializable schema"),
    );
    schemas.insert(
        "OpmlExportQuery".into(),
        serde_json::to_value(schemars::schema_for!(OpmlExportQuery)).expect("serializable schema"),
    );
    schemas.insert(
        "OpmlImportResponse".into(),
        serde_json::to_value(schemars::schema_for!(OpmlImportResponse))
            .expect("serializable schema"),
    );
    schemas.insert(
        "BootstrapAccount".into(),
        serde_json::to_value(schemars::schema_for!(BootstrapAccount)).expect("serializable schema"),
    );
    schemas.insert(
        "BootstrapResponse".into(),
        serde_json::to_value(schemars::schema_for!(BootstrapResponse))
            .expect("serializable schema"),
    );
    schemas.insert(
        "FeedDiscoverRequest".into(),
        serde_json::to_value(schemars::schema_for!(FeedDiscoverRequest))
            .expect("serializable schema"),
    );
    schemas.insert(
        "FeedPreviewArticle".into(),
        serde_json::to_value(schemars::schema_for!(FeedPreviewArticle))
            .expect("serializable schema"),
    );
    schemas.insert(
        "FeedPreviewResponse".into(),
        serde_json::to_value(schemars::schema_for!(FeedPreviewResponse))
            .expect("serializable schema"),
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&schemas).expect("serializable catalog")
    );
}
