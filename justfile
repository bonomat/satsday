set dotenv-load := true

# Default config file

CONFIG_FILE := env_var_or_default('CONFIG_FILE', 'local.config.toml')

fmt:
    just fmt-dprint

fmt-dprint:
    dprint fmt

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

run-mutinynet:
    cargo run -- --config mutinynet.config.toml start -p 12345

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
