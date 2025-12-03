#!/usr/bin/bash

migrate() {
    cargo run --bin migration -- up
}

fresh() {
    cargo run --bin migration -- fresh
}

init() {
    cargo run --bin backend -- init
    cargo run --bin backend -- create-superuser
}

generate-entity() {
    dir=crates/entity/src/entity
    rm -r $dir
    sea-orm-cli generate entity --with-serde both --date-time-crate time -o $dir
}

dev() {
    cargo watch -x "run --bin backend -- serve"
}

source .env

"$@"
