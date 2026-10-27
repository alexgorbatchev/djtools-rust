default:
    @just --list

build:
    cargo build --all-targets

test:
    cargo test --all-targets
    cargo test --doc

fmt:
    cargo fmt --package djtools

fmt-check:
    cargo fmt --package djtools -- --check

lint:
    cargo clippy --all-targets -- -D warnings

coverage:
    mkdir -p .tmp
    cargo llvm-cov --all-targets --fail-under-lines 90 --json --output-path .tmp/coverage-final.json

doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps

doctest:
    cargo test --doc

check: fmt-check lint coverage doctest doc
