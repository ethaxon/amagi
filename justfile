set dotenv-load := true
set windows-shell := ["pwsh.exe", "-NoLogo", "-ExecutionPolicy", "RemoteSigned", "-Command"]

setup:
    pnpm install
    cargo check --workspace

build-webui:
    cd apps/webui && pnpm build

build-server:
    cargo build --manifest-path apps/server/Cargo.toml --release

build-cli:
    cargo build --manifest-path apps/cli/Cargo.toml --release

build: build-webui

run-server: build
    cargo run --manifest-path apps/server/Cargo.toml --release

dev-webui:
    cd apps/webui && pnpm dev

dev-server:
    watchexec --restart --watch apps/server --watch packages/core --watch config.toml --exts rs,toml -- cargo run --manifest-path apps/server/Cargo.toml

dev: dev-server

lint-rs:
    cargo clippy --workspace -F "securitydept-creds/zone"

lint-ts:
    pnpm lint

lint: lint-rs lint-ts