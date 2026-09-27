use super::*;

pub(super) async fn sign_in<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(body): Json<CreateSessionRequest>,
) -> Result<Response, ApiFailure> {
    csrf(&s, &headers)?;
    if !s
        .repository
        .record_login_attempt(&body.username, Utc::now(), s.login_attempts_per_minute)
        .await?
    {
        return Err(ApiFailure::RateLimited);
    }
    let (raw, session) = AuthService::new(s.repository, s.auth_policy)
        .sign_in(&body.username, &body.password, Utc::now())
        .await?;
    let mut response = Json(SessionResponse {
        session_id: session.id,
        expires_at: session.expires_at,
    })
    .into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        session_cookie(&raw, s.auth_policy.session_lifetime_seconds)?,
    );
    Ok(response)
}
pub(super) async fn sign_out<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
) -> Result<Response, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    AuthService::new(s.repository, s.auth_policy)
        .sign_out(&actor.session)
        .await?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, expired_cookie());
    Ok(response)
}
pub(super) async fn create_invite<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(body): Json<CreateInviteRequest>,
) -> Result<Json<InviteResponse>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let (token, value) = AuthService::new(s.repository, s.auth_policy)
        .create_invite(&actor.account, body.username, Utc::now())
        .await?;
    Ok(Json(InviteResponse {
        url: format!(
            "{}/invite?token={}",
            s.external_origin.trim_end_matches('/'),
            token
        ),
        expires_at: value.expires_at,
    }))
}
pub(super) async fn accept_invite<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(body): Json<AcceptInviteRequest>,
) -> Result<StatusCode, ApiFailure> {
    csrf(&s, &headers)?;
    AuthService::new(s.repository.clone(), s.auth_policy)
        .accept_invite(&body.token, body.username, &body.password, Utc::now())
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn change_password<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(body): Json<ChangePasswordRequest>,
) -> Result<StatusCode, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    AuthService::new(s.repository, s.auth_policy)
        .change_password(actor.account.id, &body.current_password, &body.new_password)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn reset_password<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(body): Json<ResetPasswordRequest>,
) -> Result<StatusCode, ApiFailure> {
    csrf(&s, &headers)?;
    AuthService::new(s.repository, s.auth_policy)
        .reset_password(&body.token, &body.new_password, Utc::now())
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn create_password_reset<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(body): Json<CreatePasswordResetRequest>,
) -> Result<Json<PasswordResetResponse>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let (token, record) = AuthService::new(s.repository, s.auth_policy)
        .create_password_reset(&actor.account, &body.username, Utc::now())
        .await?;
    Ok(Json(PasswordResetResponse {
        url: format!(
            "{}/reset-password?token={}",
            s.external_origin.trim_end_matches('/'),
            token
        ),
        expires_at: record.expires_at,
    }))
}
