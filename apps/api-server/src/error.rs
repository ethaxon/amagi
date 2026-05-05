use std::net::SocketAddr;

use snafu::Snafu;

pub type Result<T, E = ServerError> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum ServerError {
    #[snafu(display("configuration error: {source}"))]
    Config { source: amagi_config::ConfigError },

    #[snafu(display("failed to bind API listener at {address}: {source}"))]
    Bind {
        address: SocketAddr,
        source: std::io::Error,
    },

    #[snafu(display("API server failed while serving requests: {source}"))]
    Serve { source: std::io::Error },
}

impl From<amagi_config::ConfigError> for ServerError {
    fn from(source: amagi_config::ConfigError) -> Self {
        Self::Config { source }
    }
}
