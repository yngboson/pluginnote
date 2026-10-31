use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("지원하지 않는 파일 포맷 버전: {0} (이 코어는 {1} 까지 지원)")]
    UnsupportedFormat(u32, u32),
    #[error("알 수 없는 객체 type: {0}")]
    UnknownType(String),
    #[error("플러그인 오류: {0}")]
    Plugin(String),
    #[error("잘못된 객체: {0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;
