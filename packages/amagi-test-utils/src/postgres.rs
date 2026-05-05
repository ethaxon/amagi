use testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner};
use testcontainers_modules::postgres;

pub struct StartedPostgres {
    _container: ContainerAsync<postgres::Postgres>,
    database_url: String,
}

impl StartedPostgres {
    pub fn database_url(&self) -> &str {
        self.database_url.as_str()
    }
}

pub async fn start_postgres(db_name: &str, user: &str, password: &str) -> StartedPostgres {
    let container = postgres::Postgres::default()
        .with_db_name(db_name)
        .with_user(user)
        .with_password(password)
        .with_tag("18-alpine")
        .start()
        .await
        .expect("postgres testcontainer starts");
    let database_url = format!(
        "postgres://{user}:{password}@{}:{}/{db_name}",
        container.get_host().await.expect("host is available"),
        container
            .get_host_port_ipv4(5432)
            .await
            .expect("postgres port is mapped")
    );

    StartedPostgres {
        _container: container,
        database_url,
    }
}

pub async fn start_amagi_postgres() -> StartedPostgres {
    start_postgres("amagi", "amagi", "amagi").await
}
