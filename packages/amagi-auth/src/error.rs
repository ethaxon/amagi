use amagi_securitydept::SecurityDeptError;
use snafu::Snafu;

use crate::principal::PrincipalError;

pub type AuthResult<T> = Result<T, AuthError>;

#[derive(Debug, Snafu)]
pub enum AuthError {
    #[snafu(display("oidc source `{source_key}` is not configured"))]
    UnknownOidcSource { source_key: String },

    #[snafu(display("database runtime is not available for auth operations"))]
    DatabaseUnavailable,

    #[snafu(display("auth principal mapping failed: {source}"))]
    Principal { source: PrincipalError },

    #[snafu(display("securitydept integration failed: {source}"))]
    SecurityDept { source: SecurityDeptError },

    #[snafu(display("bearer access token is required for this auth flow"))]
    MissingAccessToken,

    #[snafu(display("metadata redemption id is unknown or expired for source `{source_key}`"))]
    MetadataRedemptionNotFound { source_key: String },

    #[snafu(display("auth database operation failed during {action}"))]
    DatabaseQuery { action: &'static str },
}

impl AuthError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnknownOidcSource { .. } => "unknown_oidc_source",
            Self::DatabaseUnavailable => "auth_database_unavailable",
            Self::Principal { .. } => "invalid_principal",
            Self::SecurityDept { source } => source.code(),
            Self::MissingAccessToken => "auth_access_token_missing",
            Self::MetadataRedemptionNotFound { .. } => "metadata_redemption_not_found",
            Self::DatabaseQuery { .. } => "auth_database_query_failed",
        }
    }

    pub fn http_status_code(&self) -> u16 {
        match self {
            Self::UnknownOidcSource { .. } => 404,
            Self::DatabaseUnavailable => 503,
            Self::Principal { .. } => 400,
            Self::SecurityDept { source } => source.http_status(),
            Self::MissingAccessToken => 401,
            Self::MetadataRedemptionNotFound { .. } => 404,
            Self::DatabaseQuery { .. } => 500,
        }
    }

    pub fn source_key(&self) -> Option<&str> {
        match self {
            Self::UnknownOidcSource { source_key } => Some(source_key.as_str()),
            Self::SecurityDept { source } => source.source_key(),
            Self::MetadataRedemptionNotFound { source_key } => Some(source_key.as_str()),
            _ => None,
        }
    }
}

impl From<PrincipalError> for AuthError {
    fn from(source: PrincipalError) -> Self {
        Self::Principal { source }
    }
}

impl From<SecurityDeptError> for AuthError {
    fn from(source: SecurityDeptError) -> Self {
        Self::SecurityDept { source }
    }
}
