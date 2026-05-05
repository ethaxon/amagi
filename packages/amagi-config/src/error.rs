use snafu::Snafu;

pub type ConfigResult<T> = std::result::Result<T, ConfigError>;

#[derive(Debug, Snafu)]
pub enum ConfigError {
    #[snafu(display("{message}"))]
    Invalid { message: String },
}
