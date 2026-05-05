use reqwest::{Url, header::LOCATION, redirect::Policy};
use testcontainers::{ContainerAsync, runners::AsyncRunner};
use testcontainers_modules::dex::{self, PrivateClient};

pub struct StartedDex {
    _container: ContainerAsync<dex::Dex>,
    issuer_base: String,
}

impl StartedDex {
    pub fn issuer_base(&self) -> &str {
        self.issuer_base.as_str()
    }
}

pub fn auth_browser_client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .redirect(Policy::none())
        .build()
        .expect("auth browser client builds")
}

pub async fn start_dex_with_private_client(
    redirect_uri: &str,
    client_id: &str,
    client_name: &str,
    client_secret: &str,
    allow_password_grants: bool,
) -> StartedDex {
    let mut dex = dex::Dex::default()
        .with_simple_user()
        .with_client(PrivateClient {
            id: client_id.to_owned(),
            name: client_name.to_owned(),
            redirect_uris: vec![redirect_uri.to_owned()],
            secret: client_secret.to_owned(),
        });

    if allow_password_grants {
        dex = dex.with_allow_password_grants();
    }

    let container = dex.start().await.expect("dex testcontainer starts");
    let issuer_base = format!(
        "http://{}:{}",
        container.get_host().await.expect("dex host is available"),
        container
            .get_host_port_ipv4(5556)
            .await
            .expect("dex port is mapped")
    );

    StartedDex {
        _container: container,
        issuer_base,
    }
}

pub fn next_redirect_url(response: &reqwest::Response) -> Url {
    let location = response
        .headers()
        .get(LOCATION)
        .expect("redirect location header is present")
        .to_str()
        .expect("redirect location header is valid utf-8");
    response
        .url()
        .join(location)
        .or_else(|_| Url::parse(location))
        .expect("redirect location resolves")
}

pub async fn complete_dex_authorization_code_flow(
    client: &reqwest::Client,
    start_url: &str,
    redirect_uri: &str,
) -> Url {
    let mut response = client
        .get(start_url)
        .send()
        .await
        .expect("dex auth request succeeds");

    loop {
        if response.status().is_redirection() {
            let next_url = next_redirect_url(&response);
            if next_url.as_str().starts_with(redirect_uri) {
                return next_url;
            }
            response = client
                .get(next_url)
                .send()
                .await
                .expect("dex redirect follow succeeds");
            continue;
        }

        let current_url = response.url().clone();
        let body = response.text().await.expect("dex page body reads");

        if body.contains("name=\"login\"") && body.contains("name=\"password\"") {
            response = client
                .post(current_url)
                .form(&[("login", "user@example.org"), ("password", "user")])
                .send()
                .await
                .expect("dex password login succeeds");
            continue;
        }

        if body.contains("name=\"approval\" value=\"approve\"") {
            let req_id = current_url
                .query_pairs()
                .find_map(|(key, value)| (key == "req").then(|| value.into_owned()))
                .expect("approval request id is present");
            response = client
                .post(current_url)
                .form(&[("req", req_id.as_str()), ("approval", "approve")])
                .send()
                .await
                .expect("dex approval submit succeeds");
            continue;
        }

        panic!("unexpected dex page during auth flow: {current_url}");
    }
}
