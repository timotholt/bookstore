use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Session error: {0}")]
    Session(#[from] tower_sessions::session::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Askama template error: {0}")]
    Template(#[from] askama::Error),

    #[error("Not Found")]
    NotFound,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::Database(err) => {
                tracing::error!("Database error: {:?}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal database error".to_string())
            }
            AppError::Session(err) => {
                tracing::error!("Session error: {:?}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, "Session storage error".to_string())
            }
            AppError::Template(err) => {
                tracing::error!("Template rendering error: {:?}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, "Template rendering error".to_string())
            }
            AppError::Validation(msg) => {
                (StatusCode::BAD_REQUEST, msg)
            }
            AppError::NotFound => {
                (StatusCode::NOT_FOUND, "Resource not found".to_string())
            }
        };

        (status, error_message).into_response()
    }
}
