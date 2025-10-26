use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use validator::Validate;

/// Datos crudos recibidos desde un sensor ESP32
#[derive(Debug, Deserialize, Serialize, Validate, Clone)]
pub struct SensorDataInput {
    /// ID único del sensor (MAC address o ID configurado)
    #[validate(length(min = 1, max = 50))]
    pub sensor_id: String,
    
    /// Temperatura en grados Celsius
    #[validate(range(min = -50.0, max = 100.0))]
    pub temperature: f32,
    
    /// Humedad relativa en porcentaje (0-100)
    #[validate(range(min = 0.0, max = 100.0))]
    pub humidity: f32,
    
    /// Timestamp del sensor (opcional, si no se usa el del gateway)
    pub timestamp: Option<DateTime<Utc>>,
    
    /// Nivel de batería del sensor (opcional)
    #[validate(range(min = 0.0, max = 100.0))]
    pub battery_level: Option<f32>,
    
    /// Intensidad de señal WiFi (RSSI)
    pub rssi: Option<i32>,
}

/// Datos procesados y enriquecidos por el edge gateway
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProcessedSensorData {
    /// ID único de este registro
    pub id: Uuid,
    
    /// ID del sensor
    pub sensor_id: String,
    
    /// Temperatura procesada
    pub temperature: f32,
    
    /// Humedad procesada
    pub humidity: f32,
    
    /// Timestamp de recepción en el gateway
    pub gateway_timestamp: DateTime<Utc>,
    
    /// Timestamp del sensor (si está disponible)
    pub sensor_timestamp: Option<DateTime<Utc>>,
    
    /// Datos calculados por edge computing
    pub computed: ComputedMetrics,
    
    /// Estado de calidad de los datos
    pub quality: DataQuality,
    
    /// Metadatos del sensor
    pub metadata: SensorMetadata,
}

/// Métricas calculadas por edge computing
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ComputedMetrics {
    /// Índice de calor (heat index) calculado
    pub heat_index: f32,
    
    /// Punto de rocío (dew point)
    pub dew_point: f32,
    
    /// Nivel de confort (0-100)
    pub comfort_level: f32,
    
    /// Anomalía detectada (basado en histórico local)
    pub is_anomaly: bool,
    
    /// Tendencia de temperatura (-1: bajando, 0: estable, 1: subiendo)
    pub temperature_trend: i8,
    
    /// Tendencia de humedad
    pub humidity_trend: i8,
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

/// Metadatos del sensor
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SensorMetadata {
    pub battery_level: Option<f32>,
    pub rssi: Option<i32>,
    pub firmware_version: Option<String>,
}

/// Batch de múltiples lecturas
#[derive(Debug, Deserialize, Validate)]
pub struct SensorDataBatch {
    #[validate(nested)]
    #[validate(length(min = 1, max = 100))]
    pub readings: Vec<SensorDataInput>,
}

/// Estadísticas agregadas para un sensor
#[derive(Debug, Serialize)]
pub struct SensorStatistics {
    pub sensor_id: String,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub count: u32,
    pub temperature: AggregatedMetric,
    pub humidity: AggregatedMetric,
}

#[derive(Debug, Serialize)]
pub struct AggregatedMetric {
    pub min: f32,
    pub max: f32,
    pub avg: f32,
    pub std_dev: f32,
}

/// Datos enviados al servicio cloud principal
#[derive(Debug, Serialize, Clone)]
pub struct CloudPayload {
    /// ID del gateway edge
    pub gateway_id: String,
    
    /// Versión del gateway
    pub gateway_version: String,
    
    /// Datos procesados
    pub data: Vec<ProcessedSensorData>,
    
    /// Timestamp de envío
    pub sent_at: DateTime<Utc>,
    
    /// Estadísticas del batch
    pub batch_stats: BatchStatistics,
}

#[derive(Debug, Serialize, Clone)]
pub struct BatchStatistics {
    pub total_readings: u32,
    pub anomalies_detected: u32,
    pub sensors_count: u32,
    pub avg_quality_score: f32,
}