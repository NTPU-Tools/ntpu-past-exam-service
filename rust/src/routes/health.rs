use std::sync::Arc;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};

use crate::app::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/ping", get(ping))
        .route("/system-version", get(system_version))
}

async fn ping() -> Json<&'static str> {
    Json("pong")
}

async fn system_version(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "GIT_SHA": state.config.commit_sha }))
}
