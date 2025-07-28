set dotenv-load := true

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
    cargo run -- start -p 12345

balance:
    cargo run -- balance

settle:
    cargo run -- settle

address:
    cargo run -- address

game-addresses:
    cargo run -- game-addresses
