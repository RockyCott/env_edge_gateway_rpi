use crate::models::ProcessedSensorData;
use sqlx::Row;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use uuid::Uuid;

/// Capa de acceso a datos usando SQLite para almacenamiento local en edge
/// Versión 2: Soporta el nuevo modelo con header y metrics flexibles
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
        // Tabla principal de lecturas con estructura flexible
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sensor_readings (
                id TEXT PRIMARY KEY,
                
                -- Header information
                device_id TEXT NOT NULL,
                location TEXT NOT NULL,
                topic TEXT NOT NULL,
                should_requeue INTEGER NOT NULL,
                
                -- Timestamps
                gateway_timestamp TEXT NOT NULL,
                
                -- Métricas (almacenadas como JSON para flexibilidad)
                metrics_json TEXT NOT NULL,
                
                -- Métricas computadas (también JSON para flexibilidad)
                computed_json TEXT NOT NULL,
                
                -- Calidad de datos
                quality_score INTEGER NOT NULL,
                quality_issues TEXT,
                quality_corrected INTEGER NOT NULL,
                
                -- Metadatos procesados
                metrics_count INTEGER NOT NULL,
                measurement_types TEXT NOT NULL,
                
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
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_device_id ON sensor_readings(device_id);")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_location ON sensor_readings(location);")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_synced ON sensor_readings(synced);")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_gateway_timestamp ON sensor_readings(gateway_timestamp);")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_topic ON sensor_readings(topic);")
            .execute(&self.pool)
            .await?;

        tracing::info!("Migraciones de base de datos ejecutadas (v2)");
        Ok(())
    }

    /// Inserta una lectura procesada
    pub async fn insert_reading(&self, data: &ProcessedSensorData) -> anyhow::Result<()> {
        let metrics_json = serde_json::to_string(&data.metrics)?;
        let computed_json = serde_json::to_string(&data.computed)?;
        let quality_issues = serde_json::to_string(&data.quality.issues)?;
        let measurement_types = serde_json::to_string(&data.metadata.measurement_types)?;

        sqlx::query(
            r#"
            INSERT INTO sensor_readings (
                id, device_id, location, topic, should_requeue,
                gateway_timestamp, metrics_json, computed_json,
                quality_score, quality_issues, quality_corrected,
                metrics_count, measurement_types
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(data.id.to_string())
        .bind(&data.header.device_id)
        .bind(&data.header.location)
        .bind(&data.header.topic)
        .bind(data.header.should_requeue as i32)
        .bind(data.gateway_timestamp.to_rfc3339())
        .bind(metrics_json)
        .bind(computed_json)
        .bind(data.quality.score as i32)
        .bind(quality_issues)
        .bind(data.quality.corrected as i32)
        .bind(data.metadata.metrics_count as i32)
        .bind(measurement_types)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Inserta un batch de lecturas
    pub async fn insert_batch(&self, data: &[ProcessedSensorData]) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;

        for reading in data {
            let metrics_json = serde_json::to_string(&reading.metrics)?;
            let computed_json = serde_json::to_string(&reading.computed)?;
            let quality_issues = serde_json::to_string(&reading.quality.issues)?;
            let measurement_types = serde_json::to_string(&reading.metadata.measurement_types)?;

            sqlx::query(
                r#"
                INSERT INTO sensor_readings (
                    id, device_id, location, topic, should_requeue,
                    gateway_timestamp, metrics_json, computed_json,
                    quality_score, quality_issues, quality_corrected,
                    metrics_count, measurement_types
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(reading.id.to_string())
            .bind(&reading.header.device_id)
            .bind(&reading.header.location)
            .bind(&reading.header.topic)
            .bind(reading.header.should_requeue as i32)
            .bind(reading.gateway_timestamp.to_rfc3339())
            .bind(&metrics_json)
            .bind(&computed_json)
            .bind(reading.quality.score as i32)
            .bind(&quality_issues)
            .bind(reading.quality.corrected as i32)
            .bind(reading.metadata.metrics_count as i32)
            .bind(&measurement_types)
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

    /// Obtiene lecturas recientes para un dispositivo
    pub async fn get_recent_readings(
        &self,
        device_id: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<ProcessedSensorData>> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM sensor_readings
            WHERE device_id = ?
            ORDER BY gateway_timestamp DESC
            LIMIT ?
            "#,
        )
        .bind(device_id)
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

        let metrics: Vec<SensorMetric> =
            serde_json::from_str(&row.get::<String, _>("metrics_json"))?;
        let computed: ComputedMetrics =
            serde_json::from_str(&row.get::<String, _>("computed_json"))?;
        let quality_issues: Vec<String> =
            serde_json::from_str(&row.get::<String, _>("quality_issues"))?;
        let measurement_types: Vec<String> =
            serde_json::from_str(&row.get::<String, _>("measurement_types"))?;

        Ok(ProcessedSensorData {
            id: Uuid::parse_str(&row.get::<String, _>("id"))?,
            header: SensorHeader {
                user_uuid: None, // No se almacena en DB local
                device_id: row.get("device_id"),
                location: row.get("location"),
                topic: row.get("topic"),
                should_requeue: row.get::<i32, _>("should_requeue") != 0,
            },
            metrics,
            gateway_timestamp: row.get::<String, _>("gateway_timestamp").parse()?,
            computed,
            quality: DataQuality {
                score: row.get::<i32, _>("quality_score") as u8,
                issues: quality_issues,
                corrected: row.get::<i32, _>("quality_corrected") != 0,
            },
            metadata: ProcessedMetadata {
                metrics_count: row.get::<i32, _>("metrics_count") as usize,
                measurement_types,
                should_requeue: row.get::<i32, _>("should_requeue") != 0,
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
