pub fn backend_oidc_redirect_path(source_key: &str) -> String {
    format!("/api/auth/token-set/oidc/source/{source_key}/callback")
}

pub fn frontend_oidc_redirect_path(source_key: &str) -> String {
    format!("/auth/token-set/oidc/source/{source_key}/callback")
}

pub fn frontend_oidc_config_projection_path(source_key: &str) -> String {
    format!("/api/auth/token-set/oidc/source/{source_key}/config")
}

pub(crate) fn qualify_path(external_base_url: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        path.to_owned()
    } else {
        format!(
            "{}/{}",
            external_base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }
}
