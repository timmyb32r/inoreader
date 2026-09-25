#!/usr/bin/env bash
set -euo pipefail

# Real two-target acceptance for A24. It deliberately fails when Docker or the
# pinned image is unavailable; release verification must never silently skip it.
image="${INOREADER_YDB_TEST_IMAGE:-ydbplatform/local-ydb@sha256:55fdd320ee0064b9e8c628cb766d481f9be6a2c021ec8235ea8d2280fc937aaf}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture="$root/tools/fixtures/ydb_backup_restore.sql"
run_id="${RANDOM}-$$"
network="inoreader-backup-${run_id}"
source_name="inoreader-ydb-source-${run_id}"
target_name="inoreader-ydb-target-${run_id}"
artifact_dir="$(mktemp -d "${TMPDIR:-/tmp}/inoreader-backup-restore.XXXXXX")"

cleanup() {
  docker rm -f "$source_name" "$target_name" >/dev/null 2>&1 || true
  docker network rm "$network" >/dev/null 2>&1 || true
  rm -rf "$artifact_dir"
}
trap cleanup EXIT INT TERM

if ! command -v docker >/dev/null 2>&1; then
  printf 'A24 acceptance requires Docker\n' >&2
  exit 1
fi
if ! docker info >/dev/null 2>&1; then
  printf 'A24 acceptance cannot reach the Docker daemon\n' >&2
  exit 1
fi
if [[ "$image" != *@sha256:* ]]; then
  printf 'A24 acceptance requires a digest-pinned YDB image, got %s\n' "$image" >&2
  exit 1
fi
docker image inspect "$image" >/dev/null 2>&1 || docker pull "$image"
docker network create "$network" >/dev/null

start_ydb() {
  local name="$1"
  docker run -d --name "$name" --hostname "$name" --network "$network" \
    -e YDB_USE_IN_MEMORY_PDISKS=true -e GRPC_PORT=2136 -e MON_PORT=8765 \
    "$image" >/dev/null
  local attempt
  for attempt in $(seq 1 90); do
    if docker exec "$name" /ydb -e grpc://localhost:2136 -d /local --no-discovery sql -s 'SELECT 1 AS ready;' >/dev/null 2>&1; then
      return 0
    fi
    if [[ "$(docker inspect -f '{{.State.Running}}' "$name" 2>/dev/null || true)" != true ]]; then
      printf 'YDB container %s exited before becoming ready\n' "$name" >&2
      docker logs "$name" >&2 || true
      return 1
    fi
    sleep 1
  done
  printf 'YDB container %s did not become ready\n' "$name" >&2
  docker logs "$name" >&2 || true
  return 1
}

run_cli_tool() {
  docker run --rm --network "$network" \
    -v "$root:/repo:ro" -v "$artifact_dir:/artifacts" \
    --entrypoint /bin/bash "$image" -ceu \
    'mkdir -p /tmp/inoreader-bin; ln -s /ydb /tmp/inoreader-bin/ydb; export PATH="/tmp/inoreader-bin:$PATH"; exec "$@"' -- "$@"
}

query() {
  local name="$1" statement="$2"
  docker exec "$name" /ydb -e grpc://localhost:2136 -d /local --no-discovery sql -s "$statement"
}

assert_query() {
  local label="$1" name="$2" statement="$3" expected="$4" output
  output="$(query "$name" "$statement")"
  if [[ "$output" != *"$expected"* ]]; then
    printf 'A24 %s verification failed; expected %s in:\n%s\n' "$label" "$expected" "$output" >&2
    exit 1
  fi
}

start_ydb "$source_name"
docker exec -i "$source_name" /ydb -e grpc://localhost:2136 -d /local --no-discovery sql <"$fixture"
run_cli_tool /repo/tools/ydb_backup.sh --endpoint "grpc://${source_name}:2136" --database /local --output /artifacts/backup --no-discovery

start_ydb "$target_name"
target_identity="grpc://${target_name}:2136|/local"
run_cli_tool /repo/tools/ydb_restore.sh --endpoint "grpc://${target_name}:2136" --database /local --input /artifacts/backup --no-discovery --confirm-target "$target_identity"

assert_query workspace "$target_name" "SELECT COUNT(*) AS restored_workspaces FROM workspaces WHERE id='workspace-a' AND revision=7u;" '1'
assert_query state "$target_name" "SELECT document FROM subscriptions WHERE id='subscription-a';" 'maintenance'
assert_query article "$target_name" "SELECT document FROM articles WHERE id='workspace-a/article-a';" 'Durable article'
assert_query rule "$target_name" "SELECT document FROM rules WHERE id='workspace-a/rule-a';" 'markRead'
assert_query manifest "$target_name" "SELECT document FROM content_manifests WHERE id='record-a';" 'refresh-a'
assert_query fulltext "$target_name" "SELECT bytes FROM staged_content_chunks WHERE record_id='record-a' AND refresh_id='refresh-a';" '117'

# The worker leasing predicate must rediscover a job whose lease expired while
# the backup was offline. This proves restoration keeps resumable queue state.
assert_query resumable-job "$target_name" "SELECT COUNT(*) AS eligible_jobs FROM ingest_jobs WHERE (status='ready' AND run_at_ms <= 1000) OR (status='leased' AND lease_deadline_ms < 1000);" '1'

# Subscription counts are derived from primary origins in production. Rebuild
# them only after all primary samples above passed, then verify the result.
assert_query rebuilt-counter "$target_name" "SELECT subscription_id, COUNT(*) AS article_count FROM library_origins WHERE workspace_id='workspace-a' GROUP BY subscription_id;" 'subscription-a'
assert_query rebuilt-counter-value "$target_name" "SELECT COUNT(*) AS article_count FROM library_origins WHERE workspace_id='workspace-a' AND subscription_id='subscription-a';" '1'

printf 'A24 backup/restore acceptance passed with %s\n' "$image"
