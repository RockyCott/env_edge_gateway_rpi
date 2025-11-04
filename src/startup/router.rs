use super::state::AppState;
use crate::handlers;
use axum::{
    Router,
    routing::{get, post},
};
use tower_http::{compression::CompressionLayer, cors::CorsLayer, trace::TraceLayer};

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(handlers::health::health_check))
        .route("/metrics", get(handlers::metrics::get_metrics))
        .route(
            "/api/v1/sensor/data",
            post(handlers::sensor::ingest_sensor_data),
        )
        .route(
            "/api/v1/sensor/batch",
            post(handlers::sensor::ingest_batch_data),
        )
        .route("/api/v1/data/recent", get(handlers::query::get_recent_data))
        .route("/api/v1/data/stats", get(handlers::query::get_statistics))
        .with_state(state)
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}
