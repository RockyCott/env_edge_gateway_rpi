use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use thiserror::Error;

/// Errores de la aplicación
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Error de validación: {0}")]
    ValidationError(String),

    #[error("Error de base de datos: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Error de serialización: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Error interno del servidor: {0}")]
    InternalError(String),

    #[error("Recurso no encontrado: {0}")]
    NotFound(String),

    #[error("Error de configuración: {0}")]
    ConfigError(String),
}

/// Implementar conversión de anyhow::Error
impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::InternalError(err.to_string())
    }
}

/// Convertir errores en respuestas HTTP
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::ValidationError(msg) => {
                tracing::warn!("Error de validación: {}", msg);
                (StatusCode::BAD_REQUEST, msg)
            }
            AppError::DatabaseError(err) => {
                tracing::error!("Error de base de datos: {}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Error al acceder a la base de datos".to_string(),
                )
            }
            AppError::SerializationError(err) => {
                tracing::error!("Error de serialización: {}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Error al procesar datos".to_string(),
                )
            }
            AppError::InternalError(msg) => {
                tracing::error!("Error interno: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Error interno del servidor".to_string(),
                )
            }
            AppError::NotFound(msg) => {
                tracing::warn!("Recurso no encontrado: {}", msg);
                (StatusCode::NOT_FOUND, msg)
            }
            AppError::ConfigError(msg) => {
                tracing::error!("Error de configuración: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Error de configuración".to_string(),
                )
            }
        };

        let body = Json(json!({
            "status": "error",
            "message": error_message,
        }));

        (status, body).into_response()
    }
}
