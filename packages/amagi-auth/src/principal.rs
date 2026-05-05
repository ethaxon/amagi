use amagi_config::OidcIdentityClaimConfig;
use serde_json::Value;
use snafu::Snafu;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum OidcIdentityClaim {
    #[default]
    Sub,
    Email,
    Name,
    PreferredUsername,
    CustomClaim(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalOidcIdentity {
    source_key: String,
    oidc_subject: String,
    identity_claim: OidcIdentityClaim,
    oidc_identity_key: String,
    claims_snapshot: Value,
}

impl ExternalOidcIdentity {
    pub fn new(
        source_key: impl Into<String>,
        identity_claim: OidcIdentityClaim,
        claims_snapshot: Value,
    ) -> Result<Self, PrincipalError> {
        let source_key = source_key.into();
        let oidc_subject = extract_claim(&claims_snapshot, "sub")?;
        let oidc_identity_key = identity_claim.select(&claims_snapshot, &oidc_subject)?;

        Ok(Self {
            source_key,
            oidc_subject,
            identity_claim,
            oidc_identity_key,
            claims_snapshot,
        })
    }

    pub fn source_key(&self) -> &str {
        &self.source_key
    }

    pub fn oidc_subject(&self) -> &str {
        &self.oidc_subject
    }

    pub fn identity_claim(&self) -> &OidcIdentityClaim {
        &self.identity_claim
    }

    pub fn oidc_identity_key(&self) -> &str {
        &self.oidc_identity_key
    }

    pub fn claims_snapshot(&self) -> &Value {
        &self.claims_snapshot
    }

    pub fn claim_keys(&self) -> Vec<String> {
        match self.claims_snapshot.as_object() {
            Some(claims) => {
                let mut keys = claims.keys().cloned().collect::<Vec<_>>();
                keys.sort();
                keys
            }
            None => Vec::new(),
        }
    }

    pub fn audit_safe_claim_keys(&self) -> Vec<String> {
        self.claim_keys()
            .into_iter()
            .filter(|key| {
                !matches!(
                    key.as_str(),
                    "access_token" | "refresh_token" | "id_token" | "client_secret" | "code"
                )
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmagiPrincipal {
    auth_user_id: Uuid,
    user_id: Uuid,
    external_identity: ExternalOidcIdentity,
    vault_access: VaultAccessBoundary,
}

impl AmagiPrincipal {
    pub fn new(auth_user_id: Uuid, user_id: Uuid, external_identity: ExternalOidcIdentity) -> Self {
        Self {
            auth_user_id,
            user_id,
            external_identity,
            vault_access: VaultAccessBoundary::NotGranted,
        }
    }

    pub fn auth_user_id(&self) -> Uuid {
        self.auth_user_id
    }

    pub fn user_id(&self) -> Uuid {
        self.user_id
    }

    pub fn external_identity(&self) -> &ExternalOidcIdentity {
        &self.external_identity
    }

    pub fn vault_access(&self) -> VaultAccessBoundary {
        self.vault_access
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultAccessBoundary {
    NotGranted,
}

impl OidcIdentityClaim {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sub => "sub",
            Self::Email => "email",
            Self::Name => "name",
            Self::PreferredUsername => "preferred_username",
            Self::CustomClaim(_) => "custom_claim",
        }
    }

    fn select(
        &self,
        claims_snapshot: &Value,
        oidc_subject: &str,
    ) -> Result<String, PrincipalError> {
        let claim_name = match self {
            Self::Sub => return Ok(oidc_subject.to_owned()),
            Self::Email => "email",
            Self::Name => "name",
            Self::PreferredUsername => "preferred_username",
            Self::CustomClaim(claim_name) => claim_name.as_str(),
        };

        extract_claim(claims_snapshot, claim_name)
    }
}

impl From<OidcIdentityClaimConfig> for OidcIdentityClaim {
    fn from(value: OidcIdentityClaimConfig) -> Self {
        match value {
            OidcIdentityClaimConfig::Sub => Self::Sub,
            OidcIdentityClaimConfig::Email => Self::Email,
            OidcIdentityClaimConfig::Name => Self::Name,
            OidcIdentityClaimConfig::PreferredUsername => Self::PreferredUsername,
            OidcIdentityClaimConfig::CustomClaim { claim_name } => Self::CustomClaim(claim_name),
        }
    }
}

fn extract_claim(claims_snapshot: &Value, claim_name: &str) -> Result<String, PrincipalError> {
    claims_snapshot
        .get(claim_name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| PrincipalError::MissingClaim {
            claim_name: claim_name.to_owned(),
        })
}

#[derive(Debug, Snafu, PartialEq, Eq)]
pub enum PrincipalError {
    #[snafu(display("required claim `{claim_name}` is missing or empty"))]
    MissingClaim { claim_name: String },
}
