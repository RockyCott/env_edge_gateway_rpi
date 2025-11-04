use axum::{Json, extract::State};
use serde_json::{Value, json};
use validator::Validate;

use crate::{
    error::AppError,
    models::{SensorDataBatch, SensorDataInput},
    startup::state::AppState,
};

/// Handler para recibir datos individuales de un sensor
/// POST /api/v1/sensor/data
///
/// Este endpoint recibe lecturas individuales desde los ESP32
/// Aplica procesamiento edge computing y almacena localmente
pub async fn ingest_sensor_data(
    State(state): State<AppState>,
    Json(payload): Json<SensorDataInput>,
) -> Result<Json<Value>, AppError> {
    // Validar entrada
    payload
        .validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    tracing::info!(
        sensor_id = %payload.sensor_id,
        temperature = %payload.temperature,
        humidity = %payload.humidity,
        "📡 Recibiendo datos de sensor"
    );

    // Procesar datos con edge computing
    let processed = state.edge_processor.process_reading(payload).await;

    // Registrar anomalías detectadas
    if processed.computed.is_anomaly {
        tracing::warn!(
            sensor_id = %processed.sensor_id,
            "Anomalía detectada en lectura"
        );
    }

    // Almacenar en base de datos local
    state.db.insert_reading(&processed).await?;

    // Verificar si es necesario sincronizar con la nube
    let pending_count = state.db.count_pending_sync().await?;
    if pending_count >= state.config.cloud_sync_batch_size.into() {
        tracing::info!(
            pending = pending_count,
            "Iniciando sincronización con cloud"
        );

        // Disparar sincronización asíncrona
        let cloud_sync = state.cloud_sync.clone();
        let db = state.db.clone();
        tokio::spawn(async move {
            if let Err(e) = cloud_sync.sync_data(db).await {
                tracing::error!("Error en sincronización: {}", e);
            }
        });
    }

    // Responder al ESP32 con confirmación y métricas procesadas
    Ok(Json(json!({
        "status": "success",
        "message": "Datos recibidos y procesados",
        "data": {
            "id": processed.id,
            "gateway_timestamp": processed.gateway_timestamp,
            "computed_metrics": {
                "heat_index": processed.computed.heat_index,
                "dew_point": processed.computed.dew_point,
                "comfort_level": processed.computed.comfort_level,
                "is_anomaly": processed.computed.is_anomaly,
            },
            "quality_score": processed.quality.score,
        }
    })))
}

/// Handler para recibir batch de datos
/// Handler para recibir batch de datos
/// POST /api/v1/sensor/batch
///
/// Permite a los ESP32 enviar múltiples lecturas a la vez
/// Útil cuando el sensor acumula datos offline
pub async fn ingest_batch_data(
    State(state): State<AppState>,
    Json(payload): Json<SensorDataBatch>,
) -> Result<Json<Value>, AppError> {
    payload
        .validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    let batch_size = payload.readings.len();
    tracing::info!(batch_size = batch_size, "Recibiendo batch de datos");

    // Procesar todo el batch
    let processed_batch = state.edge_processor.process_batch(payload.readings).await;

    // Estadísticas del batch
    let mut anomalies = 0;
    let mut total_quality: u32 = 0;

    for data in &processed_batch {
        if data.computed.is_anomaly {
            anomalies += 1;
        }
        total_quality += data.quality.score as u32;
    }

    let avg_quality = if batch_size > 0 {
        (total_quality as f32) / (batch_size as f32)
    } else {
        0.0
    };

    // Almacenar batch en base de datos
    state.db.insert_batch(&processed_batch).await?;

    tracing::info!(
        processed = batch_size,
        anomalies = anomalies,
        avg_quality = %avg_quality,
        "Batch procesado"
    );

    // Verificar sincronización
    let pending_count = state.db.count_pending_sync().await?;
    if pending_count >= state.config.cloud_sync_batch_size.into() {
        let cloud_sync = state.cloud_sync.clone();
        let db = state.db.clone();
        tokio::spawn(async move {
            let _ = cloud_sync.sync_data(db).await;
        });
    }

    Ok(Json(json!({
        "status": "success",
        "message": "Batch procesado correctamente",
        "data": {
            "processed_count": batch_size,
            "anomalies_detected": anomalies,
            "average_quality_score": avg_quality,
            "pending_sync": pending_count,
        }
    })))
}

/// Estructura de respuesta genérica para éxito
#[derive(serde::Serialize)]
pub struct SuccessResponse<T> {
    pub status: String,
    pub message: String,
    pub data: T,
}
