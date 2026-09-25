#!/usr/bin/env bash
set -euo pipefail

endpoint=""
database=""
input=""
confirmation=""
no_discovery=false
while (($#)); do
  case "$1" in
    --endpoint) endpoint="${2:?missing endpoint}"; shift 2 ;;
    --database) database="${2:?missing database}"; shift 2 ;;
    --input) input="${2:?missing input}"; shift 2 ;;
    --confirm-target) confirmation="${2:?missing target confirmation}"; shift 2 ;;
    --no-discovery) no_discovery=true; shift ;;
    *) printf 'unknown argument: %s\n' "$1" >&2; exit 2 ;;
  esac
done

if [[ -z "$endpoint" || -z "$database" || -z "$input" ]]; then
  printf 'usage: %s --endpoint URL --database FRESH_TARGET_PATH --input BACKUP_DIRECTORY [--confirm-target ENDPOINT|DATABASE] [--no-discovery]\n' "$0" >&2
  exit 2
fi
if [[ "$database" != /* || "$database" == "/" || "$endpoint" == *$'\n'* || "$database" == *$'\n'* || "$input" == *$'\n'* ]]; then
  printf 'endpoint/input must be single-line values and database must be a non-root absolute path\n' >&2
  exit 2
fi
if ! command -v ydb >/dev/null 2>&1; then
  printf 'the official ydb CLI is required\n' >&2
  exit 1
fi
if [[ ! -d "$input" ]]; then
  printf 'backup directory is missing: %s\n' "$input" >&2
  exit 1
fi
manifest="${input}.inoreader-manifest"
if [[ ! -f "$manifest" ]]; then
  printf 'backup manifest is missing: %s\n' "$manifest" >&2
  exit 1
fi
source_database="$(sed -n 's/^source_database=//p' "$manifest")"
source_endpoint="$(sed -n 's/^source_endpoint=//p' "$manifest")"
schema_version="$(sed -n 's/^schema_version=//p' "$manifest")"
scope="$(sed -n 's/^scope=//p' "$manifest")"
backup_directory="$(sed -n 's/^backup_directory=//p' "$manifest")"
if [[ "$schema_version" != "2" || "$scope" != "/" ]]; then
  printf 'unsupported or partial backup manifest\n' >&2
  exit 1
fi
if [[ -z "$source_database" || -z "$source_endpoint" || "$backup_directory" != "$(basename "$input")" ]]; then
  printf 'backup manifest identity does not match the input directory\n' >&2
  exit 1
fi
if [[ "$source_database" == "$database" && "$source_endpoint" == "$endpoint" ]]; then
  printf 'restore target must differ from the source endpoint/database recorded in the backup\n' >&2
  exit 1
fi
target_identity="${endpoint}|${database}"
if [[ -z "$confirmation" ]]; then
  if [[ ! -t 0 ]]; then
    printf 'restore requires an interactive terminal or --confirm-target ENDPOINT|DATABASE\n' >&2
    exit 1
  fi
  printf 'Type the target identity (%s) to confirm restore into a fresh database: ' "$target_identity"
  IFS= read -r confirmation
fi
if [[ "$confirmation" != "$target_identity" ]]; then
  printf 'confirmation did not match; restore cancelled\n' >&2
  exit 1
fi

connection=(--endpoint "$endpoint" --database "$database")
if [[ "$no_discovery" == true ]]; then connection+=(--no-discovery); fi
ydb "${connection[@]}" tools restore --path "$database" --input "$input"
printf 'restore completed; verify schema, primary rows, content manifests, and pending jobs before switching traffic\n'
