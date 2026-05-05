use std::{borrow::Cow, fmt};

use securitydept_core::utils::{
    error::{ToErrorPresentation, UserRecovery},
    http::ToHttpStatus,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityDeptError {
    code: &'static str,
    message: Cow<'static, str>,
    http_status: u16,
    recovery: UserRecovery,
    source_key: Option<String>,
}

impl SecurityDeptError {
    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        self.message.as_ref()
    }

    pub fn http_status(&self) -> u16 {
        self.http_status
    }

    pub fn recovery(&self) -> UserRecovery {
        self.recovery
    }

    pub fn source_key(&self) -> Option<&str> {
        self.source_key.as_deref()
    }

    pub(crate) fn unknown_source(source_key: &str) -> Self {
        Self {
            code: "auth.oidc.source_unknown",
            message: format!("OIDC source `{source_key}` is not configured.").into(),
            http_status: 404,
            recovery: UserRecovery::RestartFlow,
            source_key: Some(source_key.to_owned()),
        }
    }

    pub(crate) fn config(
        source_key: &str,
        code: &'static str,
        message: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            http_status: 500,
            recovery: UserRecovery::ContactSupport,
            source_key: Some(source_key.to_owned()),
        }
    }

    pub(crate) fn frontend_projection(source_key: &str, source: &std::io::Error) -> Self {
        Self {
            code: "auth.oidc.frontend_projection_failed",
            message: format!(
                "Failed to build frontend OIDC config projection for source `{source_key}`: \
                 {source}"
            )
            .into(),
            http_status: 500,
            recovery: UserRecovery::Retry,
            source_key: Some(source_key.to_owned()),
        }
    }

    pub(crate) fn bearer_verifier_unavailable(source_key: &str) -> Self {
        Self {
            code: "auth.oidc.bearer_verifier_unavailable",
            message: format!(
                "OIDC source `{source_key}` does not have a configured resource-server verifier."
            )
            .into(),
            http_status: 503,
            recovery: UserRecovery::RestartFlow,
            source_key: Some(source_key.to_owned()),
        }
    }

    pub(crate) fn missing_oidc_client(source_key: &str) -> Self {
        Self {
            code: "auth.oidc.client_unavailable",
            message: format!(
                "OIDC source `{source_key}` resolved without an OIDC client instance."
            )
            .into(),
            http_status: 500,
            recovery: UserRecovery::ContactSupport,
            source_key: Some(source_key.to_owned()),
        }
    }

    pub(crate) fn runtime_build(
        source_key: &str,
        code: &'static str,
        message: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            http_status: 503,
            recovery: UserRecovery::Retry,
            source_key: Some(source_key.to_owned()),
        }
    }

    pub(crate) fn from_securitydept<E>(source_key: &str, source: &E) -> Self
    where
        E: ToErrorPresentation + ToHttpStatus,
    {
        let presentation = source.to_error_presentation();

        Self {
            code: presentation.code,
            message: presentation.message,
            http_status: source.to_http_status().as_u16(),
            recovery: presentation.recovery,
            source_key: Some(source_key.to_owned()),
        }
    }
}

impl fmt::Display for SecurityDeptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message.as_ref())
    }
}

impl std::error::Error for SecurityDeptError {}
