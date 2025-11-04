use crate::{error::AppError, startup::state::AppState};
use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Debug, Deserialize)]
pub struct RecentDataQuery {
    pub sensor_id: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    20
}

/// Handler para obtener datos recientes
/// GET /api/v1/data/recent?sensor_id=XXX&limit=20
///
/// Útil para debugging y monitoreo local
pub async fn get_recent_data(
    State(state): State<AppState>,
    Query(params): Query<RecentDataQuery>,
) -> Result<Json<Value>, AppError> {
    let data = if let Some(sensor_id) = params.sensor_id {
        state
            .db
            .get_recent_readings(&sensor_id, params.limit)
            .await?
    } else {
        // Si no se especifica sensor, retornar últimas lecturas de todos
        vec![] // Simplificado - implementar si se necesita
    };

    Ok(Json(json!({
        "status": "success",
        "count": data.len(),
        "data": data,
    })))
}

/// Handler para obtener estadísticas
/// GET /api/v1/data/stats
pub async fn get_statistics(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    let pending_sync = state.db.count_pending_sync().await?;

    Ok(Json(json!({
        "status": "success",
        "statistics": {
            "pending_sync": pending_sync,
            "gateway_id": state.config.gateway_id,
        }
    })))
}
