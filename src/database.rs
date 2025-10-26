use crate::models::ProcessedSensorData;
use sqlx::Row;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use uuid::Uuid;

/// Capa de acceso a datos usando SQLite para almacenamiento local en edge
#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Crea una nueva conexión a la base de datos SQLite
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        Ok(Self { pool })
    }

    /// Ejecuta las migraciones necesarias
    pub async fn migrate(&self) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sensor_readings (
                id TEXT PRIMARY KEY,
                sensor_id TEXT NOT NULL,
                temperature REAL NOT NULL,
                humidity REAL NOT NULL,
                gateway_timestamp TEXT NOT NULL,
                sensor_timestamp TEXT,
                
                -- Métricas computadas
                heat_index REAL NOT NULL,
                dew_point REAL NOT NULL,
                comfort_level REAL NOT NULL,
                is_anomaly INTEGER NOT NULL,
                temperature_trend INTEGER NOT NULL,
                humidity_trend INTEGER NOT NULL,
                
                -- Calidad de datos
                quality_score INTEGER NOT NULL,
                quality_issues TEXT,
                quality_corrected INTEGER NOT NULL,
                
                -- Metadatos del sensor
                battery_level REAL,
                rssi INTEGER,
                
                -- Control de sincronización
                synced INTEGER NOT NULL DEFAULT 0,
                sync_attempts INTEGER NOT NULL DEFAULT 0,
                last_sync_attempt TEXT,
                
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Índices para mejorar performance
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_sensor_id ON sensor_readings(sensor_id);")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_synced ON sensor_readings(synced);")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_gateway_timestamp ON sensor_readings(gateway_timestamp);")
            .execute(&self.pool)
            .await?;

        tracing::info!("Migraciones de base de datos ejecutadas");
        Ok(())
    }

    /// Inserta una lectura procesada
    pub async fn insert_reading(&self, data: &ProcessedSensorData) -> anyhow::Result<()> {
        let quality_issues = serde_json::to_string(&data.quality.issues)?;

        sqlx::query(
            r#"
            INSERT INTO sensor_readings (
                id, sensor_id, temperature, humidity,
                gateway_timestamp, sensor_timestamp,
                heat_index, dew_point, comfort_level,
                is_anomaly, temperature_trend, humidity_trend,
                quality_score, quality_issues, quality_corrected,
                battery_level, rssi
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(data.id.to_string())
        .bind(&data.sensor_id)
        .bind(data.temperature)
        .bind(data.humidity)
        .bind(data.gateway_timestamp.to_rfc3339())
        .bind(data.sensor_timestamp.as_ref().map(|t| t.to_rfc3339()))
        .bind(data.computed.heat_index)
        .bind(data.computed.dew_point)
        .bind(data.computed.comfort_level)
        .bind(data.computed.is_anomaly as i32)
        .bind(data.computed.temperature_trend as i32)
        .bind(data.computed.humidity_trend as i32)
        .bind(data.quality.score as i32)
        .bind(quality_issues)
        .bind(data.quality.corrected as i32)
        .bind(data.metadata.battery_level)
        .bind(data.metadata.rssi)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Inserta un batch de lecturas
    pub async fn insert_batch(&self, data: &[ProcessedSensorData]) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;

        for reading in data {
            let quality_issues = serde_json::to_string(&reading.quality.issues)?;

            sqlx::query(
                r#"
                INSERT INTO sensor_readings (
                    id, sensor_id, temperature, humidity,
                    gateway_timestamp, sensor_timestamp,
                    heat_index, dew_point, comfort_level,
                    is_anomaly, temperature_trend, humidity_trend,
                    quality_score, quality_issues, quality_corrected,
                    battery_level, rssi
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(reading.id.to_string())
            .bind(&reading.sensor_id)
            .bind(reading.temperature)
            .bind(reading.humidity)
            .bind(reading.gateway_timestamp.to_rfc3339())
            .bind(reading.sensor_timestamp.as_ref().map(|t| t.to_rfc3339()))
            .bind(reading.computed.heat_index)
            .bind(reading.computed.dew_point)
            .bind(reading.computed.comfort_level)
            .bind(reading.computed.is_anomaly as i32)
            .bind(reading.computed.temperature_trend as i32)
            .bind(reading.computed.humidity_trend as i32)
            .bind(reading.quality.score as i32)
            .bind(&quality_issues)
            .bind(reading.quality.corrected as i32)
            .bind(reading.metadata.battery_level)
            .bind(reading.metadata.rssi)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Obtiene lecturas pendientes de sincronizar
    pub async fn get_pending_sync(&self, limit: usize) -> anyhow::Result<Vec<ProcessedSensorData>> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM sensor_readings
            WHERE synced = 0
            ORDER BY gateway_timestamp ASC
            LIMIT ?
            "#,
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for row in rows {
            results.push(self.row_to_processed_data(row)?);
        }

        Ok(results)
    }

    /// Cuenta lecturas pendientes de sincronizar
    pub async fn count_pending_sync(&self) -> anyhow::Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM sensor_readings WHERE synced = 0")
            .fetch_one(&self.pool)
            .await?;

        Ok(row.get("count"))
    }

    /// Marca lecturas como sincronizadas
    pub async fn mark_as_synced(&self, ids: &[Uuid]) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;

        for id in ids {
            sqlx::query(
                r#"
                UPDATE sensor_readings
                SET synced = 1, last_sync_attempt = CURRENT_TIMESTAMP
                WHERE id = ?
                "#,
            )
            .bind(id.to_string())
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Obtiene lecturas recientes para un sensor
    pub async fn get_recent_readings(
        &self,
        sensor_id: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<ProcessedSensorData>> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM sensor_readings
            WHERE sensor_id = ?
            ORDER BY gateway_timestamp DESC
            LIMIT ?
            "#,
        )
        .bind(sensor_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for row in rows {
            results.push(self.row_to_processed_data(row)?);
        }

        Ok(results)
    }

    /// Convierte una fila de SQL a ProcessedSensorData
    fn row_to_processed_data(
        &self,
        row: sqlx::sqlite::SqliteRow,
    ) -> anyhow::Result<ProcessedSensorData> {
        use crate::models::*;

        let quality_issues: Vec<String> =
            serde_json::from_str(&row.get::<String, _>("quality_issues"))?;

        Ok(ProcessedSensorData {
            id: Uuid::parse_str(&row.get::<String, _>("id"))?,
            sensor_id: row.get("sensor_id"),
            temperature: row.get("temperature"),
            humidity: row.get("humidity"),
            gateway_timestamp: row.get::<String, _>("gateway_timestamp").parse()?,
            sensor_timestamp: row
                .get::<Option<String>, _>("sensor_timestamp")
                .map(|s| s.parse())
                .transpose()?,
            computed: ComputedMetrics {
                heat_index: row.get("heat_index"),
                dew_point: row.get("dew_point"),
                comfort_level: row.get("comfort_level"),
                is_anomaly: row.get::<i32, _>("is_anomaly") != 0,
                temperature_trend: row.get::<i32, _>("temperature_trend") as i8,
                humidity_trend: row.get::<i32, _>("humidity_trend") as i8,
            },
            quality: DataQuality {
                score: row.get::<i32, _>("quality_score") as u8,
                issues: quality_issues,
                corrected: row.get::<i32, _>("quality_corrected") != 0,
            },
            metadata: SensorMetadata {
                battery_level: row.get("battery_level"),
                rssi: row.get("rssi"),
                firmware_version: None,
            },
        })
    }

    /// Limpia lecturas antiguas ya sincronizadas
    pub async fn cleanup_old_synced(&self, days_to_keep: i64) -> anyhow::Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM sensor_readings
            WHERE synced = 1
            AND datetime(gateway_timestamp) < datetime('now', '-' || ? || ' days')
            "#,
        )
        .bind(days_to_keep)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}
