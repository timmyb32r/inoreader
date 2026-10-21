# Dependency security update — 2026-10-21

## Scope

Updated compatible Rust dependencies in both lockfiles, Vite 6.4.5, Vitest 5.0.5, and npm transitive dependencies. Removed unused testcontainers dependencies (477 → 408 packages in the main lockfile): PostgreSQL acceptance tests already use real Docker directly. Updated pinned Caddy, PostgreSQL 17 and Chromium images. PostgreSQL major, database schema and stored user data are unchanged.

## RustSec results

Audit: cargo-audit 0.22.2 against the official RustSec advisory database downloaded on 2026-10-21. No ignored advisories. Yanked-release checks were disabled for this local database audit; this is not a claim that every locked version is non-yanked.

Main lockfile: 14 vulnerability records before, 1 after. Chromium acceptance lockfile: 0 after. The enabled Linux production dependency tree contains none of the remaining vulnerability records.

| Advisory | Package before | Resolution |
| --- | --- | --- |
| [RUSTSEC-2021-0158](https://rustsec.org/advisories/RUSTSEC-2021-0158.html) | ammonia 4.2.0 | 4.2.4 |
| [RUSTSEC-2026-0373](https://rustsec.org/advisories/RUSTSEC-2026-0373.html) | ammonia 4.2.0 | 4.2.4 |
| [RUSTSEC-2021-0157](https://rustsec.org/advisories/RUSTSEC-2021-0157.html) | ammonia 4.2.0 | 4.2.4 |
| [RUSTSEC-2026-0145](https://rustsec.org/advisories/RUSTSEC-2026-0145.html) | astral-tokio-tar 0.5.6 | Removed unused dependency |
| [RUSTSEC-2026-0113](https://rustsec.org/advisories/RUSTSEC-2026-0113.html) | astral-tokio-tar 0.5.6 | Removed unused dependency |
| [RUSTSEC-2026-0066](https://rustsec.org/advisories/RUSTSEC-2026-0066.html) | astral-tokio-tar 0.5.6 | Removed unused dependency |
| [RUSTSEC-2026-0112](https://rustsec.org/advisories/RUSTSEC-2026-0112.html) | astral-tokio-tar 0.5.6 | Removed unused dependency |
| [RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071.html) | rsa 0.9.10 | Remaining; disabled optional SQLx MySQL dependency |
| [RUSTSEC-2026-0346](https://rustsec.org/advisories/RUSTSEC-2026-0346.html) | rustls 0.23.45 | 0.23.46 |
| [RUSTSEC-2026-0347](https://rustsec.org/advisories/RUSTSEC-2026-0347.html) | rustls 0.23.45 | 0.23.46 |
| [RUSTSEC-2026-0348](https://rustsec.org/advisories/RUSTSEC-2026-0348.html) | rustls 0.23.45 | 0.23.46 |
| [RUSTSEC-2026-0349](https://rustsec.org/advisories/RUSTSEC-2026-0349.html) | rustls 0.23.45 | 0.23.46 |
| [RUSTSEC-2026-0382](https://rustsec.org/advisories/RUSTSEC-2026-0382.html) | rustls-webpki 0.103.15 | 0.103.16 |
| [RUSTSEC-2026-0381](https://rustsec.org/advisories/RUSTSEC-2026-0381.html) | webpki-roots 1.0.9 | 1.0.10 |

RSA RUSTSEC-2023-0071 has no upstream fixed release. `cargo tree --locked -p inoreader --edges normal --target x86_64-unknown-linux-gnu` does not include rsa; MySQL is not enabled. The advisory remains visible to lockfile scanners. Do not enable MySQL/RSA without revisiting this.

Maintenance warnings remain for fxhash and unsafe-libyaml (through serde_yaml). These are unmaintained-package warnings, not fixed by pretending a patched release exists. Replacing the YAML parser would require separate configuration-semantics validation.

## npm results

`npm audit --json`: 5 vulnerable package entries before (2 critical, 2 high, 1 moderate), 0 after. Vite stays within major 6; Vitest moves to major 5 for the patched mocker and Tinypool dependency tree. The config imports `defineConfig` from `vitest/config`, as documented by [Vitest](https://vitest.dev/config/).

## Container audit boundary

Compose pins were refreshed from the official upstream image tags. The application image is rebuilt with `docker compose build --pull app` to refresh Node 22, Rust 1.96 and Debian bookworm base images. PostgreSQL remains 17.11; its image digest changes with the rebuilt image. No major database upgrade or schema migration is involved.

An attempted Trivy 0.76.1 OS-package audit could not complete: its 129.56 MiB database download from GHCR hit the download deadline (the default GCR mirror was unreachable). The RustSec and npm findings above are verified independently; this report does not claim a clean full OS-image vulnerability scan.

## Verification and deployment

Passed on the final dependency tree:

- `DOCKER_HOST=unix:///Users/timmyb32r/.colima/default/docker.sock just check-release`: API contracts, formatting, Clippy with warnings denied, 351 Rust tests including real PostgreSQL/backup/restore and Chromium acceptance, 218 component tests, all 122 browser cases (24.6 seconds; no retries), boundaries and operational asset guards (17 Python tests).
- `just check-affected`: 28.421 seconds; package checks 24.901 seconds, frontend typecheck 3.520 seconds.
- `npm run build`, `npm test`, `npm audit --json`.
- `git diff --check`.
- `cargo-audit audit --db <official RustSec database> --no-fetch --no-yanked --json` for both Cargo lockfiles; the root audit intentionally still returns nonzero for the recorded disabled RSA package.
- New Caddy configuration validation passed; Caddy 2.11.7, PostgreSQL 17.11.

No application hot-path code or database layout changed. Runtime performance was not benchmarked; no throughput claim is made. Removing unused test dependencies reduces the lockfile/build dependency footprint.

Production deployment completed on 2026-10-21. The candidate was validated with `docker compose run --rm --no-deps app --config /etc/inoreader/config.yaml check-config`, then deployed through graceful `docker compose stop proxy app` and `docker compose up -d --no-build --wait --wait-timeout 180`. No force kill, volume deletion, data update, configuration migration or schema migration was used. The previous app image is retained as `inoreader-app:before-deps-20261021`; the new image is also tagged `inoreader-app:deps-20261021`.

Public smoke checks after the restart: HTTPS `/` = 200; `/ready` = 204. Both return `Strict-Transport-Security: max-age=31536000`. The previous HSTS change is preserved.

Deployment evidence:

- Application image: `sha256:c518a04d59c23014be3ebb7eb6bb94230d5bda2051ab4afdc9d2ee3651a346db`.
- PostgreSQL volume identity before and after: `inoreader_postgres-data`.
- PostgreSQL, Chromium and app health checks passed; Caddy serves public HTTPS.
- Caddy version: 2.11.7; PostgreSQL version: 17.11.
- Runtime budget, users, subscriptions, article data and configuration were not edited.
