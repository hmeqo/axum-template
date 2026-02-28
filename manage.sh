#!/usr/bin/env bash

set -e

cd "$(dirname "${BASH_SOURCE[0]}")" || exit

BASE_DIR=$(pwd)

loadconfig() {
    cargo build --quiet --bin backend 2>/dev/null || true
    BIN_PATH="./target/debug/backend"
    CONFIG_JSON=$("$BIN_PATH" config)
    export DATABASE_URL=$(echo "$CONFIG_JSON" | jq -r '.database.url')
}

migrate() {
    cargo run --bin migration -- up
}

fresh() {
    cargo run --bin migration -- fresh
}

init() {
    cargo run --bin backend -- init
}

create-superuser() {
    cargo run --bin backend -- create-superuser
}

generate-entity() {
    d=crates/entity/src/entity
    rm -rf $d
    sea-orm-cli generate entity --with-serde both --date-time-crate chrono -o $d
}

dev() {
    exec cargo watch -x "run --bin backend -- serve"
}

loadconfig

"$@"
