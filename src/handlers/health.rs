use crate::startup::state::AppState;
use axum::{Json, extract::State};
use serde_json::{Value, json};

/// Handler para health check
/// GET /health
///
/// Verifica el estado del gateway y sus componentes
pub async fn health_check(State(state): State<AppState>) -> Json<Value> {
    // Verificar conexión a la base de datos
    let db_status = match state.db.count_pending_sync().await {
        Ok(_) => "healthy",
        Err(_) => "unhealthy",
    };

    // Obtener conteo de datos pendientes
    let pending_sync = state.db.count_pending_sync().await.unwrap_or(-1);

    Json(json!({
        "status": "ok",
        "gateway_id": state.config.gateway_id,
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "components": {
            "database": db_status,
            "edge_processor": "healthy",
            "cloud_sync": "healthy",
        },
        "metrics": {
            "pending_sync": pending_sync,
        }
    }))
}
