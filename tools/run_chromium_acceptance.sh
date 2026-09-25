#!/usr/bin/env bash
set -euo pipefail

readonly image="chromedp/headless-shell@sha256:2d349b544a1ea6b5b5fd7c0fe99215ff662339c57407ee2e8c0a11af93516b04"
readonly name="inoreader-chromium-acceptance-$$"
container_id=""

cleanup() {
  if [[ -n "${container_id}" ]]; then
    docker rm --force "${container_id}" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT INT TERM

command -v docker >/dev/null || { echo "Docker is required for Chromium acceptance" >&2; exit 1; }
docker info >/dev/null 2>&1 || { echo "Docker daemon is unavailable; Chromium acceptance cannot be skipped" >&2; exit 1; }

container_id="$(docker run --detach --name "${name}" \
  --security-opt no-new-privileges:true --cap-drop ALL \
  --read-only --tmpfs /tmp:size=256m,mode=1777 --tmpfs /home/chrome:size=64m,mode=0700 \
  --tmpfs /root/.cache:size=64m,mode=0700 \
  --shm-size 256m --memory 1g --cpus 2 \
  --publish 127.0.0.1:9223:9222 "${image}" \
  --remote-allow-origins='*')"

endpoint="http://127.0.0.1:9223"

healthy=false
for _ in $(seq 1 60); do
  if curl --fail --silent "${endpoint}/json/version" >/dev/null; then healthy=true; break; fi
  if ! docker inspect --format '{{.State.Running}}' "${container_id}" 2>/dev/null | grep -qx true; then
    docker logs "${container_id}" >&2 || true
    echo "Chromium container stopped before becoming healthy" >&2
    exit 1
  fi
  sleep 1
done
[[ "${healthy}" == true ]] || { docker logs "${container_id}" >&2 || true; echo "Chromium health deadline exceeded" >&2; exit 1; }

INOREADER_CHROMIUM_CDP="${endpoint}" INOREADER_CHROMIUM_CONTAINER="${container_id}" \
  cargo test --manifest-path acceptance/chromium/Cargo.toml --all-targets -- --test-threads=1
