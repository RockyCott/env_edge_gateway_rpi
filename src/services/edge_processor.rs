use crate::config::Config;
use crate::models::*;
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

/// Servicio de procesamiento edge computing
/// Realiza cálculos y análisis locales antes de enviar a la nube
pub struct EdgeProcessor {
    config: Arc<Config>,
}

impl EdgeProcessor {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }

    /// Procesa un dato individual de sensor aplicando edge computing
    pub async fn process_reading(&self, input: SensorDataInput) -> ProcessedSensorData {
        let gateway_timestamp = Utc::now();

        // Calcular métricas derivadas
        let computed = self.compute_metrics(&input);

        // Evaluar calidad de los datos
        let quality = self.assess_quality(&input, &computed);

        // Construir metadatos
        let metadata = SensorMetadata {
            battery_level: input.battery_level,
            rssi: input.rssi,
            firmware_version: None,
        };

        ProcessedSensorData {
            id: Uuid::new_v4(),
            sensor_id: input.sensor_id,
            temperature: input.temperature,
            humidity: input.humidity,
            gateway_timestamp,
            sensor_timestamp: input.timestamp,
            computed,
            quality,
            metadata,
        }
    }

    /// Calcula métricas derivadas usando algoritmos de edge computing
    fn compute_metrics(&self, input: &SensorDataInput) -> ComputedMetrics {
        // Calcular Heat Index (índice de calor)
        // Fórmula simplificada de NOAA
        let heat_index = self.calculate_heat_index(input.temperature, input.humidity);

        // Calcular Dew Point (punto de rocío)
        // Fórmula de Magnus-Tetens
        let dew_point = self.calculate_dew_point(input.temperature, input.humidity);

        // Calcular nivel de confort (basado en temp y humedad)
        let comfort_level = self.calculate_comfort_level(input.temperature, input.humidity);

        // Detectar anomalías (simplificado - en producción usaría ML local)
        let is_anomaly = self.detect_anomaly(input);

        // Calcular tendencias (requeriría histórico - simplificado aquí)
        let temperature_trend = self.calculate_trend(input.temperature);
        let humidity_trend = self.calculate_trend(input.humidity);

        ComputedMetrics {
            heat_index,
            dew_point,
            comfort_level,
            is_anomaly,
            temperature_trend,
            humidity_trend,
        }
    }

    /// Calcula el índice de calor (Heat Index)
    /// Fórmula de Rothfusz basada en NOAA
    fn calculate_heat_index(&self, temp_c: f32, humidity: f32) -> f32 {
        // Convertir a Fahrenheit para la fórmula
        let temp_f = temp_c * 9.0 / 5.0 + 32.0;

        if temp_f < 80.0 {
            return temp_c; // No aplica heat index
        }

        let t = temp_f;
        let rh = humidity;

        // Fórmula de Rothfusz
        let hi = -42.379 + 2.04901523 * t + 10.14333127 * rh
            - 0.22475541 * t * rh
            - 0.00683783 * t * t
            - 0.05481717 * rh * rh
            + 0.00122874 * t * t * rh
            + 0.00085282 * t * rh * rh
            - 0.00000199 * t * t * rh * rh;

        // Convertir de vuelta a Celsius
        (hi - 32.0) * 5.0 / 9.0
    }

    /// Calcula el punto de rocío (Dew Point)
    /// Fórmula de Magnus-Tetens
    fn calculate_dew_point(&self, temp_c: f32, humidity: f32) -> f32 {
        let a = 17.27;
        let b = 237.7;

        let alpha = ((a * temp_c) / (b + temp_c)) + (humidity / 100.0).ln();
        let dew_point = (b * alpha) / (a - alpha);

        dew_point
    }

    /// Calcula nivel de confort basado en temperatura y humedad
    /// Retorna un valor de 0 (muy incómodo) a 100 (muy cómodo)
    fn calculate_comfort_level(&self, temp_c: f32, humidity: f32) -> f32 {
        // Zona de confort ideal: 20-24°C y 40-60% humedad
        let temp_score = if temp_c >= 20.0 && temp_c <= 24.0 {
            100.0
        } else if temp_c >= 18.0 && temp_c <= 26.0 {
            80.0 - (temp_c - 22.0).abs() * 10.0
        } else {
            50.0 - (temp_c - 22.0).abs() * 5.0
        };

        let humidity_score = if humidity >= 40.0 && humidity <= 60.0 {
            100.0
        } else if humidity >= 30.0 && humidity <= 70.0 {
            80.0 - (humidity - 50.0).abs()
        } else {
            50.0 - (humidity - 50.0).abs() * 0.5
        };

        // Promedio ponderado
        let comfort = (temp_score * 0.6 + humidity_score * 0.4)
            .max(0.0)
            .min(100.0);
        comfort
    }

    /// Detecta anomalías en las lecturas
    /// Versión simplificada - en producción usaría modelos más sofisticados
    fn detect_anomaly(&self, input: &SensorDataInput) -> bool {
        // Rangos extremos que indican posible anomalía
        let temp_anomaly = input.temperature < -10.0 || input.temperature > 50.0;
        let humidity_anomaly = input.humidity < 10.0 || input.humidity > 95.0;

        // Cambios muy bruscos (requeriría lectura anterior - simplificado)
        let rapid_change = false; // Placeholder para lógica más compleja

        temp_anomaly || humidity_anomaly || rapid_change
    }

    /// Calcula tendencia simple (requeriría histórico real)
    fn calculate_trend(&self, value: f32) -> i8 {
        // Simplificado: en producción compararía con lecturas anteriores
        // Por ahora retorna 0 (estable)
        0
    }

    /// Evalúa la calidad de los datos recibidos
    fn assess_quality(&self, input: &SensorDataInput, computed: &ComputedMetrics) -> DataQuality {
        let mut score = 100u8;
        let mut issues = Vec::new();
        let corrected = false;

        // Verificar batería baja
        if let Some(battery) = input.battery_level {
            if battery < 20.0 {
                score = score.saturating_sub(10);
                issues.push(format!("Batería baja: {:.1}%", battery));
            }
        }

        // Verificar señal WiFi débil
        if let Some(rssi) = input.rssi {
            if rssi < -80 {
                score = score.saturating_sub(15);
                issues.push(format!("Señal WiFi débil: {} dBm", rssi));
            }
        }

        // Verificar anomalías
        if computed.is_anomaly {
            score = score.saturating_sub(25);
            issues.push("Lectura anómala detectada".to_string());
        }

        // Verificar valores en rangos razonables
        if input.temperature < -40.0 || input.temperature > 85.0 {
            score = score.saturating_sub(30);
            issues.push("Temperatura fuera de rango normal".to_string());
        }

        DataQuality {
            score,
            issues,
            corrected,
        }
    }

    /// Procesa un batch de lecturas en paralelo
    pub async fn process_batch(&self, inputs: Vec<SensorDataInput>) -> Vec<ProcessedSensorData> {
        let mut results = Vec::with_capacity(inputs.len());

        for input in inputs {
            results.push(self.process_reading(input).await);
        }

        results
    }
}
