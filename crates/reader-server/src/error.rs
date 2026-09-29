use super::*;

#[derive(Debug)]
pub(super) enum ApiFailure {
    Ai(reader_ai::AiError),
    Wiki(reader_wiki::Error),
    Glossary(reader_glossary::GlossaryError),
    Command(CommandError),
    Auth(AuthError),
    Repository(RepositoryError),
    Unauthorized,
    Forbidden,
    Csrf,
    Validation(&'static str),
    ExistingSubscription(bool),
    Discovery(String),
    NoWorkspace,
    RateLimited,
    Internal,
}
impl From<CommandError> for ApiFailure {
    fn from(v: CommandError) -> Self {
        Self::Command(v)
    }
}
impl From<AuthError> for ApiFailure {
    fn from(v: AuthError) -> Self {
        Self::Auth(v)
    }
}
impl From<RepositoryError> for ApiFailure {
    fn from(v: RepositoryError) -> Self {
        Self::Repository(v)
    }
}
impl From<opml::Error> for ApiFailure {
    fn from(error: opml::Error) -> Self {
        Self::Validation(match error {
            opml::Error::DeclarationsForbidden => "OPML document declarations are forbidden",
            opml::Error::InvalidDocument => "invalid OPML document",
            opml::Error::InvalidRoot => "root element must be opml",
            opml::Error::InvalidUrl => "invalid OPML URL",
            opml::Error::UnsupportedUrlScheme => "OPML URL must use HTTP or HTTPS",
            opml::Error::InvalidTitle => "invalid name",
        })
    }
}
impl IntoResponse for ApiFailure {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Wiki(error) => {
                use reader_wiki::Error;
                let status = match error {
                    Error::NotFound => StatusCode::NOT_FOUND,
                    Error::Forbidden => StatusCode::FORBIDDEN,
                    Error::Conflict | Error::NameTaken => StatusCode::CONFLICT,
                    Error::Invalid(_) => StatusCode::UNPROCESSABLE_ENTITY,
                    Error::Storage => StatusCode::INTERNAL_SERVER_ERROR,
                };
                (status, "wiki_error", error.to_string())
            }
            Self::Glossary(error) => {
                use reader_glossary::GlossaryError;
                let status = match error {
                    GlossaryError::NotFound => StatusCode::NOT_FOUND,
                    GlossaryError::Storage | GlossaryError::Configuration => {
                        StatusCode::INTERNAL_SERVER_ERROR
                    }
                    GlossaryError::Transport | GlossaryError::Protocol => StatusCode::BAD_GATEWAY,
                    GlossaryError::Conflict
                    | GlossaryError::ReceiverConflict
                    | GlossaryError::NotConnected => StatusCode::CONFLICT,
                    _ => StatusCode::UNPROCESSABLE_ENTITY,
                };
                (status, "glossary_error", error.to_string())
            }
            Self::Ai(error) => {
                use reader_ai::AiError;
                let (status, code) = match &error {
                    AiError::NotFound => (StatusCode::NOT_FOUND, "not_found"),
                    AiError::Conflict => (StatusCode::CONFLICT, "ai_conflict"),
                    AiError::Unavailable | AiError::PromptPending | AiError::MissingKey => {
                        (StatusCode::CONFLICT, "ai_unavailable")
                    }
                    AiError::Budget => (StatusCode::TOO_MANY_REQUESTS, "ai_daily_budget"),
                    AiError::RateLimit => (StatusCode::TOO_MANY_REQUESTS, "ai_rate_limit"),
                    AiError::Review => (StatusCode::UNPROCESSABLE_ENTITY, "ai_review_incomplete"),
                    AiError::Provider => (StatusCode::BAD_GATEWAY, "ai_provider_error"),
                    AiError::Storage | AiError::Encryption | AiError::Configuration => {
                        (StatusCode::INTERNAL_SERVER_ERROR, "ai_internal_error")
                    }
                    _ => (StatusCode::UNPROCESSABLE_ENTITY, "ai_invalid_request"),
                };
                (status, code, error.to_string())
            }
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "authentication required".into(),
            ),
            Self::Forbidden => (
                StatusCode::FORBIDDEN,
                "forbidden",
                "resource belongs to another account".into(),
            ),
            Self::Csrf => (
                StatusCode::FORBIDDEN,
                "csrf_rejected",
                "request origin is not allowed".into(),
            ),
            Self::Validation(v) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_request",
                v.into(),
            ),
            Self::ExistingSubscription(true) => (
                StatusCode::CONFLICT,
                "subscription_archived",
                "This source is already archived in this workspace. Restore it in Settings.".into(),
            ),
            Self::ExistingSubscription(false) => (
                StatusCode::CONFLICT,
                "subscription_exists",
                "This source is already subscribed in this workspace.".into(),
            ),
            Self::Discovery(v) => (StatusCode::UNPROCESSABLE_ENTITY, "discovery_failed", v),
            Self::NoWorkspace => (
                StatusCode::CONFLICT,
                "workspace_required",
                "account has no workspace".into(),
            ),
            Self::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "too many sign-in attempts".into(),
            ),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "internal response error".into(),
            ),
            Self::Auth(e) => {
                let (status, code) = match e {
                    AuthError::InvalidCredentials | AuthError::InvalidToken => {
                        (StatusCode::UNAUTHORIZED, "invalid_credentials")
                    }
                    AuthError::AdminRequired => (StatusCode::FORBIDDEN, "admin_required"),
                    AuthError::UsernameExists => (StatusCode::CONFLICT, "username_exists"),
                    AuthError::Repository(RepositoryError::NotFound) => {
                        (StatusCode::NOT_FOUND, "not_found")
                    }
                    AuthError::Repository(RepositoryError::Conflict) => {
                        (StatusCode::CONFLICT, "conflict")
                    }
                    AuthError::Repository(RepositoryError::Storage(_)) | AuthError::Hashing => {
                        (StatusCode::INTERNAL_SERVER_ERROR, "storage_error")
                    }
                    _ => (StatusCode::UNPROCESSABLE_ENTITY, "invalid_request"),
                };
                (status, code, e.to_string())
            }
            Self::Repository(e) => repository_failure(e),
            Self::Command(e) => match e {
                CommandError::InvalidReason(_) => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid_reason",
                    e.to_string(),
                ),
                CommandError::WorkspaceArchived => {
                    (StatusCode::CONFLICT, "workspace_archived", e.to_string())
                }
                CommandError::Repository(r) => repository_failure(r),
            },
        };
        (status, Json(ApiError { code, message })).into_response()
    }
}
