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

build-extension-chrome:
    cd apps/extension-web && pnpm build:chrome

build-extension-firefox:
    cd apps/extension-web && pnpm build:firefox

build-extension-safari:
    cd apps/extension-web && pnpm build:safari

smoke-extension-chrome:
    cd apps/extension-web && pnpm smoke:chrome

build-api:
    cargo build --manifest-path apps/api-server/Cargo.toml

build: build-api build-dashboard build-extension

dev-api:
    watchexec --watch apps/api-server --watch packages --watch dev --exts rs,toml -- cargo run --manifest-path apps/api-server/Cargo.toml -- --config dev/amagi.config.local.toml

dev-dashboard:
    cd apps/dashboard-web && pnpm dev

dev-extension-chrome:
    cd apps/extension-web && pnpm dev:chrome

dev-extension-firefox:
    cd apps/extension-web && pnpm dev:firefox

dev-extension-safari:
    cd apps/extension-web && pnpm dev:safari

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
