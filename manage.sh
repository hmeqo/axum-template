#!/usr/bin/env bash
set -e

cd "$(dirname "${BASH_SOURCE[0]}")" || exit

migrate() {
    cargo run --bin toasty-cli -- migration apply
}

fresh() {
    cargo run --bin toasty-cli -- migration reset --skip-migrations
    cargo run --bin toasty-cli -- migration apply
}

migration-generate() {
    cargo run --bin toasty-cli -- migration generate "$@"
}

init() {
    cargo run --bin backend -- init
}

create-superuser() {
    cargo run --bin backend -- create-superuser
}

dev() {
    exec cargo watch -x "run --bin backend"
}

"$@"
