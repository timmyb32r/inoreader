_default:
    @just --list

# Ordinary development gate: compilation/typechecking only.
check-affected *args:
    python3 scripts/test_affected.py {{args}}

test-affected-dry *args:
    python3 scripts/test_affected.py --dry-run {{args}}

check: check-affected

crate-boundaries:
    python3 scripts/check_crate_boundaries.py

source-inventory:
    ruby tools/validate_source_inventory.rb source-inventory/inventory.json

operational-assets:
    python3 tools/check_operational_assets.py

seed-preview account workspace output="seed-manifest.json":
    python3 tools/prepare_seed_manifest.py --inventory source-inventory/inventory.json --account-id {{account}} --workspace-id {{workspace}} --output {{output}}

seed-validate manifest:
    python3 tools/validate_seed_manifest.py {{manifest}}

# Complete release gate, including hermetic Docker acceptance tests.
check-release:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo test --workspace --all-targets --all-features
    bash tools/run_chromium_acceptance.sh
    cd web && npm test
    cd web && npm run test:e2e
    ./tools/test_ydb_backup_restore.sh
    python3 scripts/check_crate_boundaries.py
    ruby tools/validate_source_inventory.rb source-inventory/inventory.json
    python3 tools/check_operational_assets.py
