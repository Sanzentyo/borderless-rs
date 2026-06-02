use thiserror::Error;

pub type CoreResult<T> = Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("empty value for {0}")]
    Empty(&'static str),
    #[error("invalid rectangle: left={left}, top={top}, right={right}, bottom={bottom}")]
    InvalidRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    },
    #[error("no monitor contains window {0:?}")]
    MonitorNotFound(crate::types::Hwnd),
    #[error("regex error: {0}")]
    Regex(#[from] regex::Error),
    #[error("unsupported transition: {0}")]
    Transition(&'static str),
}
