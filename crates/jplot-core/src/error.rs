use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum JplotError {
    #[error("missing aesthetic `{0}`")]
    MissingAes(String),
    #[error("column `{0}` not found in data")]
    ColumnNotFound(String),
    #[error("data is empty")]
    EmptyData,
    #[error("invalid spec: {0}")]
    InvalidSpec(String),
    #[error("json error: {0}")]
    Json(String),
    #[error("render error: {0}")]
    Render(String),
    #[error("io error: {0}")]
    Io(String),
}

impl From<serde_json::Error> for JplotError {
    fn from(e: serde_json::Error) -> Self {
        JplotError::Json(e.to_string())
    }
}

impl From<std::io::Error> for JplotError {
    fn from(e: std::io::Error) -> Self {
        JplotError::Io(e.to_string())
    }
}
