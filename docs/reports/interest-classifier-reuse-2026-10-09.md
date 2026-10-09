# Reuse unchanged interest classifications

The source-document invalidation trigger can mark a prediction dirty even when the classifier's complete provider input remains unchanged. The worker now checks retained successful replies before reserving money or calling DeepSeek.

Reuse requires the same owner, workspace, article, profile revision and exact serialized provider body (system prompt, title, article content, model and generation parameters). A valid active claim is checked before lookup. Interrupted, unsuccessful or invalid predictions are not reused. Changed input is classified normally. Existing paid history supplies the cache; no schema or budget changes are needed.

PostgreSQL regression coverage checks invalidation followed by identical-input reuse, changed-input miss, foreign-owner rejection and profile-revision isolation.

Verification: `just check-release` passed (real PostgreSQL and backup/restore acceptance, 218 frontend tests, 121 browser tests), followed by `just check-affected`. Deployed image `inoreader-app:interest-cache-20261009`, `sha256:198771aaa8baeeb29b31e6756f9233329bf0b3b4ab6539d2084a0c7757c89603`; running image and public liveness verified. Previous unified-timer image retained. No schema, budget or configuration changes.
