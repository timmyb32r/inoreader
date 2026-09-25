#!/usr/bin/env bash
set -euo pipefail

endpoint=""
database=""
output=""
no_discovery=false
while (($#)); do
  case "$1" in
    --endpoint) endpoint="${2:?missing endpoint}"; shift 2 ;;
    --database) database="${2:?missing database}"; shift 2 ;;
    --output) output="${2:?missing output}"; shift 2 ;;
    --no-discovery) no_discovery=true; shift ;;
    *) printf 'unknown argument: %s\n' "$1" >&2; exit 2 ;;
  esac
done

if [[ -z "$endpoint" || -z "$database" || -z "$output" ]]; then
  printf 'usage: %s --endpoint URL --database PATH --output NEW_DIRECTORY [--no-discovery]\n' "$0" >&2
  exit 2
fi
if [[ "$database" != /* || "$database" == "/" || "$endpoint" == *$'\n'* || "$database" == *$'\n'* || "$output" == *$'\n'* ]]; then
  printf 'endpoint/output must be single-line values and database must be a non-root absolute path\n' >&2
  exit 2
fi
if ! command -v ydb >/dev/null 2>&1; then
  printf 'the official ydb CLI is required\n' >&2
  exit 1
fi
if [[ -e "$output" ]]; then
  printf 'refusing to overwrite existing backup path: %s\n' "$output" >&2
  exit 1
fi
manifest="${output}.inoreader-manifest"
if [[ -e "$manifest" ]]; then
  printf 'refusing to overwrite existing backup manifest: %s\n' "$manifest" >&2
  exit 1
fi

connection=(--endpoint "$endpoint" --database "$database")
if [[ "$no_discovery" == true ]]; then connection+=(--no-discovery); fi
ydb "${connection[@]}" tools dump --path / --output "$output"
created_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
{
  printf 'schema_version=2\n'
  printf 'created_at_utc=%s\n' "$created_at"
  printf 'source_database=%s\n' "$database"
  printf 'source_endpoint=%s\n' "$endpoint"
  printf 'scope=/\n'
  printf 'backup_directory=%s\n' "$(basename "$output")"
} >"$manifest"
printf 'backup completed: %s\n' "$output"
