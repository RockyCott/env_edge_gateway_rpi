use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use validator::Validate;

/// Datos crudos recibidos desde un sensor ESP32
#[derive(Debug, Serialize, Deserialize, Validate, Clone)]
pub struct SensorDataInput {
    /// Header con información del dispositivo
    #[validate(nested)]
    pub header: SensorHeader,

    /// Métricas del sensor (flexibles)
    #[validate(length(min = 1))]
    pub metrics: Vec<SensorMetric>,
}

/// Header con información del dispositivo
#[derive(Debug, Deserialize, Serialize, Validate, Clone)]
pub struct SensorHeader {
    /// UUID del usuario (será reemplazado por el gateway)
    #[serde(rename = "userUUID")]
    pub user_uuid: Option<String>,

    /// ID del dispositivo
    #[validate(length(min = 1, max = 50))]
    #[serde(rename = "deviceId")]
    pub device_id: String,

    /// Ubicación del sensor
    #[validate(length(min = 1, max = 200))]
    pub location: String,

    /// Topic del mensaje
    pub topic: String,

    /// Si debe reencolar el mensaje
    #[serde(rename = "shouldRequeue")]
    pub should_requeue: bool,
}

/// Métrica individual del sensor
#[derive(Debug, Deserialize, Serialize, Validate, Clone)]
pub struct SensorMetric {
    /// Nombre de la medición (Temperature, Humidity, etc.)
    #[validate(length(min = 1, max = 100))]
    pub measurement: String,

    /// Valor de la medición
    pub value: f32,
}

/// Datos procesados y enriquecidos por el edge gateway
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProcessedSensorData {
    /// ID único de este registro
    pub id: Uuid,

    /// Header del sensor
    pub header: SensorHeader,

    /// Métricas originales
    pub metrics: Vec<SensorMetric>,

    /// Timestamp de recepción en el gateway
    pub gateway_timestamp: DateTime<Utc>,

    /// Datos calculados por edge computing
    pub computed: ComputedMetrics,

    /// Estado de calidad de los datos
    pub quality: DataQuality,

    /// Metadatos adicionales extraídos
    pub metadata: ProcessedMetadata,
}

/// Métricas calculadas por edge computing
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ComputedMetrics {
    /// Índice de calor (si hay temperatura y humedad)
    pub heat_index: Option<f32>,

    /// Punto de rocío (si hay temperatura y humedad)
    pub dew_point: Option<f32>,

    /// Nivel de confort (0-100) si aplica
    pub comfort_level: Option<f32>,

    /// Anomalía detectada (basado en histórico local)
    pub is_anomaly: bool,

    /// Estadísticas adicionales calculadas
    pub stats: HashMap<String, f32>,
}

/// Calidad de los datos recibidos
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DataQuality {
    /// Calidad general (0-100)
    pub score: u8,

    /// Razones de baja calidad
    pub issues: Vec<String>,

    /// Datos fueron corregidos/interpolados
    pub corrected: bool,
}

/// Metadatos procesados
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProcessedMetadata {
    /// Número de métricas recibidas
    pub metrics_count: usize,

    /// Tipos de mediciones detectadas
    pub measurement_types: Vec<String>,

    /// Si el mensaje debe reencolar
    pub should_requeue: bool,
}

/// Batch de múltiples lecturas
#[derive(Debug, Deserialize, Validate)]
pub struct SensorDataBatch {
    #[validate(length(min = 1, max = 100))]
    #[validate(nested)]
    pub readings: Vec<SensorDataInput>,
}

/// Estadísticas agregadas para un sensor
#[derive(Debug, Serialize)]
pub struct SensorStatistics {
    pub device_id: String,
    pub location: String,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub count: u32,
    pub metrics_summary: HashMap<String, MetricSummary>,
}

#[derive(Debug, Serialize)]
pub struct MetricSummary {
    pub measurement: String,
    pub min: f32,
    pub max: f32,
    pub avg: f32,
    pub count: u32,
}

/// Datos enviados al servicio cloud principal via MQTT
#[derive(Debug, Serialize, Clone)]
pub struct CloudPayload {
    /// Header actualizado con información del gateway
    pub header: CloudHeader,

    /// Métricas originales más las computadas
    pub metrics: Vec<SensorMetric>,

    /// Timestamp de envío
    pub sent_at: DateTime<Utc>,

    /// Información de calidad
    pub quality: DataQuality,
}

/// Header para enviar al cloud (con UUID del gateway)
#[derive(Debug, Serialize, Clone)]
pub struct CloudHeader {
    #[serde(rename = "userUUID")]
    pub user_uuid: String,

    #[serde(rename = "deviceId")]
    pub device_id: String,

    pub location: String,

    pub topic: String,

    #[serde(rename = "shouldRequeue")]
    pub should_requeue: bool,

    /// ID del gateway que procesó
    pub gateway_id: String,
}

/// Estadísticas de batch para cloud
#[derive(Debug, Serialize, Clone)]
pub struct CloudBatchStats {
    pub total_readings: u32,
    pub anomalies_detected: u32,
    pub devices_count: u32,
    pub avg_quality_score: f32,
    pub gateway_id: String,
}
