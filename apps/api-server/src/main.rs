mod app;
mod error;
mod http;

use amagi_config::ApiServerConfig;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt};

use crate::error::{Result, ServerError};

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let config = ApiServerConfig::load()?;
    let bind_addr = config.bind_addr()?;
    let state = app::build_state(config).await;
    let router = app::build_app(state);

    let listener = TcpListener::bind(bind_addr)
        .await
        .map_err(|source| ServerError::Bind {
            address: bind_addr,
            source,
        })?;

    info!(%bind_addr, "starting amagi API server");
    axum::serve(listener, router)
        .await
        .map_err(|source| ServerError::Serve { source })
}

fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    fmt().with_env_filter(env_filter).init();
}
