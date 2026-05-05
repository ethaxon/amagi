set dotenv-load := true
set windows-shell := ["pwsh.exe", "-NoLogo", "-ExecutionPolicy", "RemoteSigned", "-Command"]

setup:
    mise install
    pnpm install
    cargo check --workspace --all-features

build-dashboard:
    cd apps/dashboard-web && pnpm build

build-extension:
    cd apps/extension-web && pnpm build

build-api:
    cargo build --manifest-path apps/api-server/Cargo.toml

build: build-api build-dashboard build-extension

dev-api:
    watchexec --watch apps/api-server --watch packages --exts rs,toml -- cargo run --manifest-path apps/api-server/Cargo.toml

dev-dashboard:
    cd apps/dashboard-web && pnpm dev

dev-extension:
    cd apps/extension-web && pnpm dev

dev-deps:
    docker compose -f devdeps.compose.yaml up

dev-deps-clean:
    docker compose -f devdeps.compose.yaml down -v

dev:
    zellij --layout zellij-dev.kdl

lint-rs:
    cargo clippy --workspace --all-features --all-targets

lint-ts:
    pnpm lint

lint: lint-rs lint-ts

fix-rs:
    cargo clippy --workspace --all-features --all-targets --fix --allow-dirty

fix-ts:
    pnpm lint-fix

fix: fix-rs fix-ts

test-ts:
    pnpm test

test-rs:
    cargo test --workspace --all-features

test: test-rs test-ts

typecheck:
    pnpm typecheck
