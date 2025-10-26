use crate::AppState;
use axum::{Json, extract::State};
use serde_json::{Value, json};

/// Handler para métricas del sistema
/// GET /metrics
///
/// Retorna métricas de operación del gateway
pub async fn get_metrics(State(state): State<AppState>) -> Json<Value> {
    let pending_sync = state.db.count_pending_sync().await.unwrap_or(0);

    // Aquí podrías agregar más métricas como:
    // - Tasa de lecturas por minuto
    // - Sensores activos
    // - Uso de memoria
    // - Uso de disco
    // - etc.

    Json(json!({
        "gateway_id": state.config.gateway_id,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "metrics": {
            "pending_sync_count": pending_sync,
            "sync_batch_size": state.config.cloud_sync_batch_size,
            "sync_interval_secs": state.config.cloud_sync_interval_secs,
        }
    }))
}
