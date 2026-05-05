use axum::Router;

use crate::app::AppState;

pub mod auth;
pub mod dashboard;
pub mod health;
pub mod sync;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(auth::router())
        .merge(dashboard::router())
        .merge(sync::router())
        .merge(health::router())
}
