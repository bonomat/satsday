set dotenv-load := true

# Default config file

CONFIG_FILE := env_var_or_default('CONFIG_FILE', 'local.config.toml')

mod mainnet
mod mutiny

fmt:
    just fmt-dprint
    just fmt-frontend

fmt-dprint:
    dprint fmt

fmt-frontend:
    #!/usr/bin/env bash
    set -euxo pipefail
    cd frontend/satsday
    pnpm biome format --write .

clippy:
    cargo clippy --all-targets --all-features -- -D warnings

db-prepare:
    cargo sqlx prepare

db-add-migration args="":
    sqlx migrate add --source ./migrations -r {{ args }}

db-run-migration:
    sqlx migrate run --source=./migrations

db-revert-migration:
    sqlx migrate revert --source=./migrations

run:
    cargo run -- --config {{ CONFIG_FILE }} start -p 12345

balance:
    cargo run -- --config {{ CONFIG_FILE }} balance

settle:
    cargo run -- --config {{ CONFIG_FILE }} settle

address:
    cargo run -- --config {{ CONFIG_FILE }} address

game-addresses:
    cargo run -- --config {{ CONFIG_FILE }} game-addresses

send address amount:
    cargo run -- --config {{ CONFIG_FILE }} send {{ address }} {{ amount }}

run-frontend:
    #!/bin/bash
    set -e  # Exit on any error
    cd frontend/satsday
    TRANSACTION_CHECK_INTERVAL_SECONDS=3 VITE_MAX_PAYOUT_SATS=100000 VITE_API_BASE_URL=http://localhost:12345 pnpm run dev
