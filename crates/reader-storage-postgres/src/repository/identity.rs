use super::*;

#[async_trait::async_trait]
impl reader_application::IdentityRepository for PostgresRepository {
    async fn account(&self, id: AccountId) -> Result<AccountRecord, RepositoryError> {
        self.read("accounts", id.as_uuid().to_string()).await
    }
    async fn account_by_username(&self, u: &str) -> Result<AccountRecord, RepositoryError> {
        let document: String =
            sqlx::query_scalar("SELECT document FROM username_reservations WHERE id = $1")
                .bind(u)
                .fetch_optional(&self.pool)
                .await
                .map_err(storage)?
                .ok_or(RepositoryError::NotFound)?;
        self.account(AccountId::from_uuid(
            Uuid::parse_str(&document).map_err(storage)?,
        ))
        .await
    }
    async fn save_account(&self, e: Option<u64>, v: AccountRecord) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        if e.is_none() {
            sqlx::query(
                "INSERT INTO username_reservations (id, revision, document) VALUES ($1, 0, $2)",
            )
            .bind(&v.username)
            .bind(v.id.as_uuid().to_string())
            .execute(&mut *tx)
            .await
            .map_err(|_| RepositoryError::Conflict)?;
        }
        if cas_tx(
            &mut tx,
            "accounts",
            v.id.as_uuid().to_string(),
            e,
            v.revision,
            &v,
        )
        .await?
            != 1
        {
            return Err(RepositoryError::Conflict);
        }
        tx.commit().await.map_err(storage)
    }
    async fn create_account_and_workspace(
        &self,
        a: AccountRecord,
        w: Workspace,
    ) -> Result<(), RepositoryError> {
        if w.owner() != a.id || w.revision() != 0 {
            return Err(RepositoryError::Storage(
                "initial workspace must belong to the new account at revision zero".into(),
            ));
        }
        w.validate(self.reason_policy).map_err(storage)?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        sqlx::query(
            "INSERT INTO username_reservations (id, revision, document) VALUES ($1, 0, $2)",
        )
        .bind(&a.username)
        .bind(a.id.as_uuid().to_string())
        .execute(&mut *tx)
        .await
        .map_err(|_| RepositoryError::Conflict)?;
        if cas_tx(
            &mut tx,
            "accounts",
            a.id.as_uuid().to_string(),
            None,
            a.revision,
            &a,
        )
        .await?
            != 1
            || cas_tx(
                &mut tx,
                "workspaces",
                w.id().as_uuid().to_string(),
                None,
                w.revision(),
                &w,
            )
            .await?
                != 1
        {
            return Err(RepositoryError::Conflict);
        }
        tx.commit().await.map_err(storage)
    }
    async fn invite_by_token_hash(&self, h: &str) -> Result<InviteRecord, RepositoryError> {
        self.read("invites", h.to_owned()).await
    }
    async fn save_invite(&self, e: Option<u64>, v: InviteRecord) -> Result<(), RepositoryError> {
        self.cas(
            "invites",
            v.token_hash.clone(),
            String::new(),
            e,
            v.revision,
            &v,
        )
        .await
    }
    async fn session_by_verifier_hash(&self, h: &str) -> Result<SessionRecord, RepositoryError> {
        self.read("sessions", h.to_owned()).await
    }
    async fn save_session(&self, e: Option<u64>, v: SessionRecord) -> Result<(), RepositoryError> {
        self.cas(
            "sessions",
            v.verifier_hash.clone(),
            v.account_id.as_uuid().to_string(),
            e,
            v.revision,
            &v,
        )
        .await
    }
    async fn delete_session(&self, h: &str, e: u64) -> Result<(), RepositoryError> {
        let n = sqlx::query("DELETE FROM sessions WHERE id = $1 AND revision = $2")
            .bind(h)
            .bind(e as i64)
            .execute(&self.pool)
            .await
            .map_err(storage)?
            .rows_affected();
        if n != 1 {
            return Err(RepositoryError::Conflict);
        }
        Ok(())
    }
    async fn password_reset_by_token_hash(
        &self,
        h: &str,
    ) -> Result<PasswordResetRecord, RepositoryError> {
        self.read("password_resets", h.to_owned()).await
    }
    async fn save_password_reset(
        &self,
        e: Option<u64>,
        v: PasswordResetRecord,
    ) -> Result<(), RepositoryError> {
        self.cas(
            "password_resets",
            v.token_hash.clone(),
            v.account_id.as_uuid().to_string(),
            e,
            v.revision,
            &v,
        )
        .await
    }
    async fn consume_invite_create_account_and_workspace(
        &self,
        expected: u64,
        invite: InviteRecord,
        account: AccountRecord,
        workspace: Workspace,
    ) -> Result<(), RepositoryError> {
        workspace.validate(self.reason_policy).map_err(storage)?;
        if workspace.owner() != account.id {
            return Err(storage(
                "initial workspace must belong to the invited account",
            ));
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let username_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM username_reservations WHERE id = $1)")
                .bind(&account.username)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage)?;
        if username_exists
            || cas_tx(
                &mut tx,
                "invites",
                invite.token_hash.clone(),
                Some(expected),
                invite.revision,
                &invite,
            )
            .await?
                != 1
        {
            return Err(RepositoryError::Conflict);
        }
        sqlx::query("INSERT INTO username_reservations (id,revision,document) VALUES ($1,0,$2)")
            .bind(&account.username)
            .bind(account.id.as_uuid().to_string())
            .execute(&mut *tx)
            .await
            .map_err(|_| RepositoryError::Conflict)?;
        if cas_tx(
            &mut tx,
            "accounts",
            account.id.as_uuid().to_string(),
            None,
            account.revision,
            &account,
        )
        .await?
            != 1
            || cas_tx(
                &mut tx,
                "workspaces",
                workspace.id().as_uuid().to_string(),
                None,
                workspace.revision(),
                &workspace,
            )
            .await?
                != 1
        {
            return Err(RepositoryError::Conflict);
        }
        tx.commit().await.map_err(storage)
    }
    async fn consume_reset_and_update_account(
        &self,
        expected_reset: u64,
        reset: PasswordResetRecord,
        expected_account: u64,
        account: AccountRecord,
    ) -> Result<(), RepositoryError> {
        if reset.account_id != account.id {
            return Err(storage(
                "password reset account does not match updated account",
            ));
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        if cas_tx(
            &mut tx,
            "password_resets",
            reset.token_hash.clone(),
            Some(expected_reset),
            reset.revision,
            &reset,
        )
        .await?
            != 1
            || cas_tx(
                &mut tx,
                "accounts",
                account.id.as_uuid().to_string(),
                Some(expected_account),
                account.revision,
                &account,
            )
            .await?
                != 1
        {
            return Err(RepositoryError::Conflict);
        }
        tx.commit().await.map_err(storage)
    }
    async fn record_login_attempt(
        &self,
        u: &str,
        at: DateTime<Utc>,
        limit: u32,
    ) -> Result<bool, RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        #[derive(Serialize, Deserialize)]
        struct LoginAttempts {
            timestamps: Vec<DateTime<Utc>>,
            revision: u64,
        }
        let row: Option<(i64, String)> =
            sqlx::query_as("SELECT revision,document FROM login_attempts WHERE id=$1 FOR UPDATE")
                .bind(u)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?;
        let (mut value, expected) = match row {
            Some((revision, document)) => (
                serde_json::from_str::<LoginAttempts>(&document).map_err(storage)?,
                Some(u64::try_from(revision).map_err(storage)?),
            ),
            None => (
                LoginAttempts {
                    timestamps: Vec::new(),
                    revision: 0,
                },
                None,
            ),
        };
        let cutoff = at - chrono::Duration::minutes(1);
        value.timestamps.retain(|v| *v > cutoff);
        if value.timestamps.len() >= limit as usize {
            return Ok(false);
        }
        value.timestamps.push(at);
        value.revision = expected.map_or(0, |v| v + 1);
        if cas_tx(
            &mut tx,
            "login_attempts",
            u.to_owned(),
            expected,
            value.revision,
            &value,
        )
        .await?
            != 1
        {
            return Err(RepositoryError::Conflict);
        }
        tx.commit().await.map_err(storage)?;
        Ok(true)
    }
}
