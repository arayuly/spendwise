use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Validation(String),
    #[error("Требуется авторизация")]
    Unauthorized,
    #[error("Доступ запрещён")]
    Forbidden,
    #[error("Ресурс не найден")]
    NotFound,
    #[error("{0}")]
    Conflict(String),
    #[error("Слишком много запросов. Попробуйте через минуту")]
    RateLimited,
    #[error("Размер запроса превышает 64 КиБ")]
    PayloadTooLarge,
    #[error("Требуется Content-Type: application/json")]
    UnsupportedMediaType,
    #[error("Внутренняя ошибка сервера")]
    Internal,
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        match &error {
            sqlx::Error::RowNotFound => Self::NotFound,
            sqlx::Error::Database(e) if e.is_unique_violation() => {
                Self::Conflict("Такая запись уже существует".into())
            }
            sqlx::Error::Database(e) if e.is_foreign_key_violation() => {
                Self::Conflict("Запись связана с другими данными".into())
            }
            sqlx::Error::Database(e) if e.code().as_deref() == Some("23514") => {
                Self::Validation("Данные нарушают ограничения".into())
            }
            _ => {
                tracing::error!(error = %error, "database operation failed");
                Self::Internal
            }
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = match self {
            Self::Validation(_) => (StatusCode::UNPROCESSABLE_ENTITY, "VALIDATION_ERROR"),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED"),
            Self::Forbidden => (StatusCode::FORBIDDEN, "FORBIDDEN"),
            Self::NotFound => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            Self::Conflict(_) => (StatusCode::CONFLICT, "CONFLICT"),
            Self::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMITED"),
            Self::PayloadTooLarge => (StatusCode::PAYLOAD_TOO_LARGE, "PAYLOAD_TOO_LARGE"),
            Self::UnsupportedMediaType => {
                (StatusCode::UNSUPPORTED_MEDIA_TYPE, "UNSUPPORTED_MEDIA_TYPE")
            }
            Self::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
        };
        let mut response = (
            status,
            Json(json!({"error":{"code":code,"message":self.to_string(),"details":{}}})),
        )
            .into_response();
        if status == StatusCode::TOO_MANY_REQUESTS {
            response
                .headers_mut()
                .insert("retry-after", axum::http::HeaderValue::from_static("60"));
        }
        response
    }
}

/// JSON extractor with the same public error envelope as domain failures.
pub struct AppJson<T>(pub T);
impl<S, T> axum::extract::FromRequest<S> for AppJson<T>
where
    S: Send + Sync,
    T: serde::de::DeserializeOwned,
{
    type Rejection = AppError;
    async fn from_request(request: axum::extract::Request, state: &S) -> AppResult<Self> {
        let Json(value) =
            Json::<T>::from_request(request, state)
                .await
                .map_err(|error| match error.status() {
                    StatusCode::PAYLOAD_TOO_LARGE => AppError::PayloadTooLarge,
                    StatusCode::UNSUPPORTED_MEDIA_TYPE => AppError::UnsupportedMediaType,
                    _ => AppError::Validation("Некорректное JSON-тело запроса".into()),
                })?;
        Ok(Self(value))
    }
}
