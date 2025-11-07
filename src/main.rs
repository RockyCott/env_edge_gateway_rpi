use axum::{
    Router,
    routing::{get, post},
};
use std::sync::Arc;
use tower_http::{compression::CompressionLayer, cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod database;
mod error;
mod handlers;
mod models;
mod services;

use config::Config;
use database::Database;
use services::edge_processor::EdgeProcessor;
use services::{cloud_sync::CloudSync, mqtt_handler::MqttHandler};

/// Estado compartido de la aplicación
#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub edge_processor: Arc<EdgeProcessor>,
    pub cloud_sync: Arc<CloudSync>,
    pub config: Arc<Config>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Inicializar logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "env_edge_gateway_rpi=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Iniciando IoT Gateway Edge Computing");

    // Cargar configuración
    let config = Arc::new(Config::load()?);
    tracing::info!("Configuración cargada");

    // Inicializar base de datos SQLite local
    let db = Database::new(&config.database_url).await?;
    db.migrate().await?;
    tracing::info!("Base de datos inicializada");

    // Inicializar servicios
    let edge_processor = Arc::new(EdgeProcessor::new(config.clone()));
    let cloud_sync = Arc::new(CloudSync::new(config.clone()));
    tracing::info!("Servicios de edge computing inicializados");

    // Iniciar tarea de sincronización periódica con la nube
    let cloud_sync_clone = cloud_sync.clone();
    let db_clone = db.clone();
    tokio::spawn(async move {
        cloud_sync_clone.start_sync_task(db_clone).await;
    });

    // Inicializar y arrancar MQTT handler
    tracing::info!("Iniciando MQTT Handler...");
    let mqtt_handler = MqttHandler::new(
        config.clone(),
        db.clone(),
        edge_processor.clone(),
        cloud_sync.clone(),
    )
    .await?;

    let mqtt_task = mqtt_handler.start().await;
    tracing::info!("MQTT Handler iniciado");

    // Construir estado compartido
    let app_state = AppState {
        db,
        edge_processor,
        cloud_sync,
        config: config.clone(),
    };

    // Construir router con todas las rutas
    let app = Router::new()
        // Health check y métricas
        .route("/health", get(handlers::health::health_check))
        .route("/metrics", get(handlers::metrics::get_metrics))
        // Rutas de ingesta de datos
        .route(
            "/api/v1/sensor/data",
            post(handlers::sensor::ingest_sensor_data),
        )
        .route(
            "/api/v1/sensor/batch",
            post(handlers::sensor::ingest_batch_data),
        )
        // Rutas de consulta (para debugging/monitoreo)
        .route("/api/v1/data/recent", get(handlers::query::get_recent_data))
        .route("/api/v1/data/stats", get(handlers::query::get_statistics))
        // Estado compartido
        .with_state(app_state)
        // Middlewares
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    // Obtener dirección de bind
    let addr = format!("0.0.0.0:{}", config.http_port.unwrap_or(3000));
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!("Servidor HTTP escuchando en {}", addr);
    tracing::info!(
        "Broker MQTT: {}:{}",
        config.mqtt_broker_host,
        config.mqtt_broker_port
    );
    tracing::info!("Topics MQTT:");
    tracing::info!("   - sensors/+/data (publicar datos individuales)");
    tracing::info!("   - sensors/+/batch (publicar batches)");
    tracing::info!("   - sensors/+/processed (respuestas del gateway)");
    tracing::info!("Sistema listo para recibir datos de sensores IoT");

    // Iniciar servidor
    let http_server = axum::serve(listener, app);

    tokio::select! {
        result = http_server => {
            if let Err(e) = result {
                tracing::error!("Error en el servidor HTTP: {}", e);
            }
        }
        _ = mqtt_task => {
            tracing::error!("El MQTT Handler ha finalizado inesperadamente");
        }
    }

    Ok(())
}
