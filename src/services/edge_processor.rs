use crate::config::Config;
use crate::models::*;
use chrono::Utc;
use std::collections::HashMap;
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

        // Extraer temperatura y humedad si existen en las métricas
        let temp_metric = input
            .metrics
            .iter()
            .find(|m| m.measurement.to_lowercase() == "temperature");
        let hum_metric = input.metrics.iter().find(|m| {
            m.measurement.to_lowercase() == "humidity" || m.measurement.to_lowercase() == "humedad"
        });

        // Calcular métricas derivadas
        let computed = self.compute_metrics(&input.metrics, temp_metric, hum_metric);

        // Evaluar calidad de los datos
        let quality = self.assess_quality(&input, &computed);

        // Construir metadatos
        let metadata = ProcessedMetadata {
            metrics_count: input.metrics.len(),
            measurement_types: input
                .metrics
                .iter()
                .map(|m| m.measurement.clone())
                .collect(),
            should_requeue: input.header.should_requeue,
        };

        ProcessedSensorData {
            id: Uuid::new_v4(),
            header: input.header,
            metrics: input.metrics,
            gateway_timestamp,
            computed,
            quality,
            metadata,
        }
    }

    /// Calcula métricas derivadas usando algoritmos de edge computing
    fn compute_metrics(
        &self,
        metrics: &[SensorMetric],
        temp_metric: Option<&SensorMetric>,
        hum_metric: Option<&SensorMetric>,
    ) -> ComputedMetrics {
        let mut stats = HashMap::new();

        // Calcular Heat Index y Dew Point si hay temperatura y humedad
        let (heat_index, dew_point, comfort_level) =
            if let (Some(temp), Some(hum)) = (temp_metric, hum_metric) {
                let hi = self.calculate_heat_index(temp.value, hum.value);
                let dp = self.calculate_dew_point(temp.value, hum.value);
                let cl = self.calculate_comfort_level(temp.value, hum.value);
                (Some(hi), Some(dp), Some(cl))
            } else {
                (None, None, None)
            };

        // Calcular estadísticas básicas para cada métrica
        for metric in metrics {
            // Aquí podrías agregar más estadísticas si tienes histórico
            stats.insert(format!("{}_current", metric.measurement), metric.value);
        }

        // Detectar anomalías
        let is_anomaly = self.detect_anomaly(metrics, temp_metric, hum_metric);

        ComputedMetrics {
            heat_index,
            dew_point,
            comfort_level,
            is_anomaly,
            stats,
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
    fn detect_anomaly(
        &self,
        metrics: &[SensorMetric],
        temp_metric: Option<&SensorMetric>,
        hum_metric: Option<&SensorMetric>,
    ) -> bool {
        // Rangos extremos para temperatura
        if let Some(temp) = temp_metric {
            if temp.value < -10.0 || temp.value > 50.0 {
                return true;
            }
        }

        // Rangos extremos para humedad
        if let Some(hum) = hum_metric {
            if hum.value < 10.0 || hum.value > 95.0 {
                return true;
            }
        }

        // Detectar valores extremos en cualquier métrica
        for metric in metrics {
            // Valores muy negativos o muy altos podrían ser anomalías
            if metric.value.is_nan() || metric.value.is_infinite() {
                return true;
            }

            // Rangos específicos por tipo de medición
            match metric.measurement.to_lowercase().as_str() {
                "distance" | "distancia" => {
                    if metric.value < 0.0 || metric.value > 10000.0 {
                        return true;
                    }
                }
                "voltage" | "voltaje" => {
                    if metric.value < 0.0 || metric.value > 50.0 {
                        return true;
                    }
                }
                _ => {
                    // Detección genérica
                    if metric.value.abs() > 10000.0 {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Evalúa la calidad de los datos recibidos
    fn assess_quality(&self, input: &SensorDataInput, computed: &ComputedMetrics) -> DataQuality {
        let mut score = 100u8;
        let mut issues = Vec::new();
        let corrected = false;

        // Verificar que haya métricas
        if input.metrics.is_empty() {
            score = score.saturating_sub(50);
            issues.push("No hay métricas en el mensaje".to_string());
        }

        // Verificar anomalías
        if computed.is_anomaly {
            score = score.saturating_sub(25);
            issues.push("Lectura anómala detectada".to_string());
        }

        // Verificar valores NaN o infinitos
        for metric in &input.metrics {
            if metric.value.is_nan() {
                score = score.saturating_sub(30);
                issues.push(format!("Valor NaN en métrica: {}", metric.measurement));
            }
            if metric.value.is_infinite() {
                score = score.saturating_sub(30);
                issues.push(format!("Valor infinito en métrica: {}", metric.measurement));
            }
        }

        // Verificar location válido
        if input.header.location.trim().is_empty() {
            score = score.saturating_sub(10);
            issues.push("Ubicación vacía o inválida".to_string());
        }

        DataQuality {
            score,
            issues,
            corrected,
        }
    }

    /// Procesa un batch de lecturas
    pub async fn process_batch(&self, inputs: Vec<SensorDataInput>) -> Vec<ProcessedSensorData> {
        let mut results = Vec::with_capacity(inputs.len());

        for input in inputs {
            results.push(self.process_reading(input).await);
        }

        results
    }
}
