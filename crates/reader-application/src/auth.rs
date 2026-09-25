use crate::{
    AccountRecord, InviteRecord, PasswordResetRecord, ReaderRepository, RepositoryError,
    SessionRecord,
};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params, Version,
};
use chrono::{DateTime, Duration, Utc};
use rand::{rngs::OsRng, RngCore};
use reader_core::{AccountId, Workspace, WorkspaceId};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Argon2idPolicy {
    memory_kib: u32,
    time_cost: u32,
    parallelism: u32,
}

impl Argon2idPolicy {
    /// Validated Argon2id resource policy. Values are embedded in every password hash.
    pub fn new(memory_kib: u32, time_cost: u32, parallelism: u32) -> Result<Self, AuthError> {
        Params::new(memory_kib, time_cost, parallelism, None)
            .map_err(|_| AuthError::InvalidHashPolicy)?;
        Ok(Self {
            memory_kib,
            time_cost,
            parallelism,
        })
    }
    fn engine(self) -> Result<Argon2<'static>, AuthError> {
        Ok(Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(self.memory_kib, self.time_cost, self.parallelism, None)
                .map_err(|_| AuthError::InvalidHashPolicy)?,
        ))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AuthPolicy {
    pub session_lifetime_seconds: u64,
    pub invite_lifetime_seconds: u64,
    pub reset_lifetime_seconds: u64,
    pub argon2id: Argon2idPolicy,
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("authentication token is invalid or expired")]
    InvalidToken,
    #[error("username is invalid")]
    InvalidUsername,
    #[error("password must contain at least 12 characters")]
    WeakPassword,
    #[error("administrator privileges are required")]
    AdminRequired,
    #[error("username already exists")]
    UsernameExists,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("password hashing failed")]
    Hashing,
    #[error("invalid Argon2id resource policy")]
    InvalidHashPolicy,
}

pub struct AuthService<R> {
    repository: Arc<R>,
    policy: AuthPolicy,
}

impl<R: ReaderRepository> AuthService<R> {
    pub fn new(repository: Arc<R>, policy: AuthPolicy) -> Self {
        Self { repository, policy }
    }

    pub async fn authenticate(
        &self,
        raw_token: &str,
        now: DateTime<Utc>,
    ) -> Result<(SessionRecord, AccountRecord), AuthError> {
        let session = self
            .repository
            .session_by_verifier_hash(&token_hash(raw_token))
            .await
            .map_err(|e| match e {
                RepositoryError::NotFound => AuthError::InvalidToken,
                other => other.into(),
            })?;
        if session.expires_at <= now {
            return Err(AuthError::InvalidToken);
        }
        let account = self.repository.account(session.account_id).await?;
        if session.account_auth_revision != account.auth_revision {
            return Err(AuthError::InvalidToken);
        }
        Ok((session, account))
    }

    pub async fn sign_in(
        &self,
        username: &str,
        password: &str,
        now: DateTime<Utc>,
    ) -> Result<(String, SessionRecord), AuthError> {
        validate_username(username)?;
        let account = self
            .repository
            .account_by_username(username)
            .await
            .map_err(|e| match e {
                RepositoryError::NotFound => AuthError::InvalidCredentials,
                other => other.into(),
            })?;
        verify_password_async(password, &account.password_hash, self.policy.argon2id).await?;
        let raw = random_token();
        let record = SessionRecord {
            id: Uuid::new_v4(),
            verifier_hash: token_hash(&raw),
            account_id: account.id,
            account_auth_revision: account.auth_revision,
            expires_at: now + seconds(self.policy.session_lifetime_seconds)?,
            revision: 0,
        };
        self.repository.save_session(None, record.clone()).await?;
        Ok((raw, record))
    }

    pub async fn sign_out(&self, session: &SessionRecord) -> Result<(), AuthError> {
        self.repository
            .delete_session(&session.verifier_hash, session.revision)
            .await?;
        Ok(())
    }

    pub async fn create_invite(
        &self,
        actor: &AccountRecord,
        username: String,
        now: DateTime<Utc>,
    ) -> Result<(String, InviteRecord), AuthError> {
        if !actor.admin {
            return Err(AuthError::AdminRequired);
        }
        validate_username(&username)?;
        match self.repository.account_by_username(&username).await {
            Err(RepositoryError::NotFound) => {}
            Ok(_) => return Err(AuthError::UsernameExists),
            Err(e) => return Err(e.into()),
        }
        let raw = random_token();
        let invite = InviteRecord {
            id: Uuid::new_v4(),
            token_hash: token_hash(&raw),
            username,
            created_by: actor.id,
            expires_at: now + seconds(self.policy.invite_lifetime_seconds)?,
            consumed_at: None,
            revision: 0,
        };
        self.repository.save_invite(None, invite.clone()).await?;
        Ok((raw, invite))
    }

    pub async fn accept_invite(
        &self,
        raw: &str,
        username: String,
        password: &str,
        now: DateTime<Utc>,
    ) -> Result<AccountRecord, AuthError> {
        validate_username(&username)?;
        validate_password(password)?;
        let mut invite = self
            .repository
            .invite_by_token_hash(&token_hash(raw))
            .await
            .map_err(|e| match e {
                RepositoryError::NotFound => AuthError::InvalidToken,
                other => other.into(),
            })?;
        if invite.expires_at <= now || invite.consumed_at.is_some() || invite.username != username {
            return Err(AuthError::InvalidToken);
        }
        match self.repository.account_by_username(&username).await {
            Err(RepositoryError::NotFound) => {}
            Ok(_) => return Err(AuthError::UsernameExists),
            Err(e) => return Err(e.into()),
        }
        let account = AccountRecord {
            id: AccountId::new(),
            username,
            password_hash: hash_password_async(password, self.policy.argon2id).await?,
            admin: false,
            auth_revision: 0,
            revision: 0,
        };
        let expected = invite.revision;
        invite.consumed_at = Some(now);
        invite.revision += 1;
        let workspace = Workspace::new(WorkspaceId::new(), account.id, "Reading".to_owned());
        self.repository
            .consume_invite_create_account_and_workspace(
                expected,
                invite,
                account.clone(),
                workspace,
            )
            .await?;
        Ok(account)
    }

    pub async fn change_password(
        &self,
        account_id: AccountId,
        current: &str,
        new: &str,
    ) -> Result<(), AuthError> {
        validate_password(new)?;
        let mut account = self.repository.account(account_id).await?;
        verify_password_async(current, &account.password_hash, self.policy.argon2id).await?;
        let expected = account.revision;
        account.password_hash = hash_password_async(new, self.policy.argon2id).await?;
        account.auth_revision += 1;
        account.revision += 1;
        self.repository
            .save_account(Some(expected), account)
            .await?;
        Ok(())
    }

    pub async fn reset_password(
        &self,
        raw: &str,
        new: &str,
        now: DateTime<Utc>,
    ) -> Result<(), AuthError> {
        validate_password(new)?;
        let mut reset = self
            .repository
            .password_reset_by_token_hash(&token_hash(raw))
            .await
            .map_err(|e| match e {
                RepositoryError::NotFound => AuthError::InvalidToken,
                other => other.into(),
            })?;
        if reset.expires_at <= now || reset.consumed_at.is_some() {
            return Err(AuthError::InvalidToken);
        }
        let mut account = self.repository.account(reset.account_id).await?;
        let account_revision = account.revision;
        account.password_hash = hash_password_async(new, self.policy.argon2id).await?;
        account.auth_revision += 1;
        account.revision += 1;
        let reset_revision = reset.revision;
        reset.consumed_at = Some(now);
        reset.revision += 1;
        self.repository
            .consume_reset_and_update_account(reset_revision, reset, account_revision, account)
            .await?;
        Ok(())
    }

    pub async fn create_password_reset(
        &self,
        actor: &AccountRecord,
        username: &str,
        now: DateTime<Utc>,
    ) -> Result<(String, PasswordResetRecord), AuthError> {
        if !actor.admin {
            return Err(AuthError::AdminRequired);
        }
        validate_username(username)?;
        let account = self.repository.account_by_username(username).await?;
        let raw = random_token();
        let record = PasswordResetRecord {
            id: Uuid::new_v4(),
            token_hash: token_hash(&raw),
            account_id: account.id,
            expires_at: now + seconds(self.policy.reset_lifetime_seconds)?,
            consumed_at: None,
            revision: 0,
        };
        self.repository
            .save_password_reset(None, record.clone())
            .await?;
        Ok((raw, record))
    }

    pub async fn bootstrap_admin(
        &self,
        username: String,
        password: &str,
    ) -> Result<AccountRecord, AuthError> {
        validate_username(&username)?;
        validate_password(password)?;
        match self.repository.account_by_username(&username).await {
            Err(RepositoryError::NotFound) => {}
            Ok(_) => return Err(AuthError::UsernameExists),
            Err(e) => return Err(e.into()),
        }
        let account = AccountRecord {
            id: AccountId::new(),
            username,
            password_hash: hash_password_async(password, self.policy.argon2id).await?,
            admin: true,
            auth_revision: 0,
            revision: 0,
        };
        let workspace = Workspace::new(WorkspaceId::new(), account.id, "Personal".to_owned());
        self.repository
            .create_account_and_workspace(account.clone(), workspace)
            .await?;
        Ok(account)
    }
}

fn seconds(value: u64) -> Result<Duration, AuthError> {
    i64::try_from(value)
        .ok()
        .and_then(Duration::try_seconds)
        .ok_or(AuthError::InvalidToken)
}
fn validate_username(value: &str) -> Result<(), AuthError> {
    if value.is_empty() || value.trim() != value || value.len() > 254 {
        Err(AuthError::InvalidUsername)
    } else {
        Ok(())
    }
}
fn validate_password(value: &str) -> Result<(), AuthError> {
    if value.chars().count() < 12 {
        Err(AuthError::WeakPassword)
    } else {
        Ok(())
    }
}
fn hash_password(value: &str, policy: Argon2idPolicy) -> Result<String, AuthError> {
    validate_password(value)?;
    policy
        .engine()?
        .hash_password(value.as_bytes(), &SaltString::generate(&mut OsRng))
        .map(|v| v.to_string())
        .map_err(|_| AuthError::Hashing)
}
fn verify_password(value: &str, encoded: &str, policy: Argon2idPolicy) -> Result<(), AuthError> {
    let parsed = PasswordHash::new(encoded).map_err(|_| AuthError::InvalidCredentials)?;
    policy
        .engine()?
        .verify_password(value.as_bytes(), &parsed)
        .map_err(|_| AuthError::InvalidCredentials)
}
async fn hash_password_async(value: &str, policy: Argon2idPolicy) -> Result<String, AuthError> {
    let value = value.to_owned();
    tokio::task::spawn_blocking(move || hash_password(&value, policy))
        .await
        .map_err(|_| AuthError::Hashing)?
}
async fn verify_password_async(
    value: &str,
    encoded: &str,
    policy: Argon2idPolicy,
) -> Result<(), AuthError> {
    let value = value.to_owned();
    let encoded = encoded.to_owned();
    tokio::task::spawn_blocking(move || verify_password(&value, &encoded, policy))
        .await
        .map_err(|_| AuthError::Hashing)?
}
fn random_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex(&bytes)
}
pub fn token_hash(raw: &str) -> String {
    hex(&Sha256::digest(raw.as_bytes()))
}
fn hex(bytes: &[u8]) -> String {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(H[(byte >> 4) as usize] as char);
        out.push(H[(byte & 15) as usize] as char);
    }
    out
}

#[cfg(test)]
#[path = "tests/auth.rs"]
mod tests;
