use serde::Deserialize;
use std::env;

/// Configuración de la aplicación
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// ID único del gateway edge
    pub gateway_id: String,

    /// URL de la base de datos SQLite local
    pub database_url: String,

    /// URL del servicio cloud principal
    pub cloud_service_url: String,

    /// API key para autenticación con el cloud
    pub cloud_api_key: String,

    /// Tamaño del batch antes de sincronizar
    pub cloud_sync_batch_size: u32,

    /// Intervalo de sincronización periódica (segundos)
    pub cloud_sync_interval_secs: u64,

    /// Días para mantener datos sincronizados localmente
    pub data_retention_days: i64,
}

impl Config {
    /// Carga la configuración desde variables de entorno
    pub fn load() -> anyhow::Result<Self> {
        // Cargar archivo .env si existe
        dotenv::dotenv().ok();

        let config = Config {
            gateway_id: env::var("GATEWAY_ID")
                .unwrap_or_else(|_| format!("gateway-{}", uuid::Uuid::new_v4())),

            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://sensor_data.db".to_string()),

            cloud_service_url: env::var("CLOUD_SERVICE_URL")
                .expect("CLOUD_SERVICE_URL debe estar configurada"),

            cloud_api_key: env::var("CLOUD_API_KEY").expect("CLOUD_API_KEY debe estar configurada"),

            cloud_sync_batch_size: env::var("CLOUD_SYNC_BATCH_SIZE")
                .unwrap_or_else(|_| "50".to_string())
                .parse()?,

            cloud_sync_interval_secs: env::var("CLOUD_SYNC_INTERVAL_SECS")
                .unwrap_or_else(|_| "300".to_string()) // 5 minutos por defecto
                .parse()?,

            data_retention_days: env::var("DATA_RETENTION_DAYS")
                .unwrap_or_else(|_| "7".to_string())
                .parse()?,
        };

        Ok(config)
    }
}
