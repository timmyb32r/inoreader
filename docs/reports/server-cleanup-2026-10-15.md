# Server cleanup — 2026-10-15

User explicitly authorized deleting backups, old build/deployment directories, and unnecessary artifacts on the Reader server.

Deleted the verified unmounted directories `/home/timmyb32r/inoreader-backups`, `/home/timmyb32r/inoreader/backups`, and `/home/timmyb32r/inoreader/.inoreader-state`. Removed unused `inoreader-app` image tags while retaining every image referenced by any container. Pruned unused rebuildable Docker builder cache. Existing database/Caddy volumes, current source, prompts, configuration and secrets were preserved.

Added configurable JSON log rotation to all four compose services: `CONTAINER_LOG_MAX_SIZE` defaults to `50m`, `CONTAINER_LOG_MAX_FILES` defaults to `3`. Gracefully stopped the application and recreated services from existing images, retaining their volumes; old container logs were removed with their containers.

Validation: local `docker-compose config --quiet`, remote `docker compose config --quiet`, `git diff --check`; application, PostgreSQL and Chromium healthy; PostgreSQL accepts connections; public `/ready` returns 204. Disk after cleanup: 34 GiB used, 141 GiB available (20% used), compared with 100% at incident start. No application-code changes or build were needed.
