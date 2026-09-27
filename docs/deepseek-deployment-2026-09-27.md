# DeepSeek deployment — 2026-09-27

Candidate04 and factual review v3 are deployed at
<https://inoreader.duckdns.org> on `158.160.186.87`. The authenticated public
smoke passed at `2026-09-27T00:42:52.725176+00:00`. The owner explicitly accepted
the current candidate and waived further questionnaires; the separate
[acceptance record](../prompts/reading-data-news/deployment-approval.json)
preserves that decision without changing the failed research factual gate or
inventing another human score.

Only account `541affad-f1b0-4dd1-90de-7f41b689d4e6` is enabled, with
`prompt_approved: true`. The example configuration retains its disabled defaults.
The profile has no provider key: the owner enters it in **Profile → DeepSeek API
key**, then uses **Summarize** on an article with available full text. No key from
`~/.deepseek` was imported. The profile supports a provider balance query after
the key is configured.

## Released artifact and configuration

- Running image: `sha256:35da77df56ccc5a772aafe4784be1a9996eff66dbae1a9d729ac20c44cfa2dbd`.
- Tags: `inoreader-app:deepseek-candidate04` and `inoreader-app:latest`.
- Prompt: `reading-data-news-2026-09-27-candidate-04`, draft revision v17.
- Review: `reading-data-news-factual-review-2026-09-27-v3`.
- Existing server configuration was checked against its backup and preserved
  byte for byte before appending the new AI block. Existing credentials remain
  unchanged. The AI block uses the example's validated settings, with only the
  owner allowlist and explicit prompt approval enabled.
- Runtime prompts live under `/home/timmyb32r/inoreader/prompts/reading-data-news`.
  Research exports, experiment outputs, browser state and private source corpora
  were excluded from deployment.

The build completed with frontend typechecking/Vite and release Rust compilation.
Only the application was recreated with `docker compose up -d --no-deps
--no-build app`; PostgreSQL, Chromium and Caddy were not restarted. Startup created
the three AI tables: `ai_profiles`, `ai_chats`, `ai_operations`.

The host lacked Docker's Buildx plugin. The package-manager dry run showed one
new package and no upgrades/removals; `docker-buildx=0.30.1-0ubuntu1~24.04.1` was
installed from the host's signed Ubuntu repository. Service restarts were
deferred. The image then built using the existing BuildKit cache.

## Backup and encryption key

Before deployment, the original configuration, Compose file, Dockerfile and a
complete custom-format PostgreSQL dump were saved under:

`/home/timmyb32r/inoreader-backups/deepseek-20260927T002716Z`

The directory is root-owned mode0700; backup files are mode0600. The dump is
872,897,329 bytes. `pg_restore --list` parsed it successfully and showed account
and subscription table data. No production restore was attempted.

The previous image is retained as
`inoreader-app:before-deepseek-20260927t002716z`, image
`sha256:8c82111b952f2b341dfacec819eb4df7970d8274c643ec1332b93c83f0d2617c`.
Application rollback must preserve the current database and AI master key;
restoring the pre-deployment database would discard subsequent user data and is
not an automatic rollback step.

A new 32-byte random encryption key was created once at
`/home/timmyb32r/inoreader/secrets/ai-encryption-key`, owned by application
UID/GID10001 with mode0400. Compose mounts it at
`/run/secrets/ai-encryption-key`. Never regenerate this key while encrypted
profile credentials exist.

A byte-verified separate server copy is stored at
`/home/timmyb32r/.local/share/inoreader-key-backups/ai-encryption-key-20260927T002716Z`.
Its parent is root-owned mode0700 and the file is mode0400. Key bytes were never
printed. An optional additional export to the local workspace was rejected by
automatic approval review because deployment authorization did not explicitly
authorize exporting this production secret. The export was not performed or
bypassed. The separate server copy supports this deployment but is not an
off-host disaster-recovery backup.

## Verification and its limits

The final implementation passed `just check-affected` (4.419s) and
`just check-release` (101.212s): Rust202, separate Chromium extraction3,
frontend127 and browser30 checks, including real PostgreSQL/YDB and Docker
backup/restore acceptance. Research tooling72 and blind-review browser5 checks
also passed. See the chronological
[verification record](tasks/deepseek-verification.md).

On the server, Compose validation, application configuration validation and real
application startup passed. The public smoke verified:

- `/live` and `/ready`: HTTP204, 0.248s and0.061s respectively.
- Unauthenticated AI profile: HTTP401.
- Owner AI profile: HTTP200 in0.087s, with the exact missing-key availability
  reason, confirming the server integration is enabled for this owner.
- Wrong-origin balance POST: HTTP403 before any provider request.
- Eleven browser checks: immediate pending feedback on opening the chat,
  duplicate-activation protection, generation/send disabled without a key,
  pointer dragging and keyboard movement, minimize/expand, unchanged background
  toolbar geometry, protected empty profile key field, disabled key actions,
  restored focus on close, and no page errors or unexpected UI writes.

The smoke used a temporary five-minute owner session. Its raw cookie remained in
memory; only the digest was written to session storage. Cleanup deleted the exact
temporary session and confirmed its absence. No article or read-state mutation
was sent. All three AI tables remained empty.

Private evidence is preserved under
`.inoreader-state/deployments/deepseek-20260927/`, final report
`production-smoke-49a37f79-a6ee-4bea-b015-e0e8f0ca6b8c.json`. The earlier smoke
incorrectly required the initially selected article to have full text available;
that unrelated precondition was removed from the missing-key test. Its failure
report and successful session cleanup remain preserved. Product code was not
changed to accommodate the smoke.

No paid conversation using a saved production profile key was exercised, because
the owner has not entered that key. Real provider streaming was independently
tested through the production adapter before deployment: both draft and review
completed and passed envelope, exact-title and quote checks in93.386s total,
costing USD0.046344936. Durable conversation/retry/restart behavior is covered by
real PostgreSQL tests. These checks do not guarantee factual accuracy: the fresh
research cohort retained three detected factual-error cases among18 formatted
outputs from20 planned sources. The owner accepted this limitation without a
further questionnaire.
