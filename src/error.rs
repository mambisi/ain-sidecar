use thiserror::Error;

#[derive(Debug, Error)]
pub enum NodeRunnerError {
    #[error("Docker operation failed: {0}")]
    DockerError(#[from] bollard::errors::Error),
    #[error("Serde JSON error: {0}")]
    SerdeJsonError(#[from] serde_json::error::Error),
    #[error("FailedToExec")]
    FailedToExec,
    #[error("ParseError")]
    ParseError,
    #[error("Filesystem operation failed: {0}")]
    FilesystemError(#[from] fs_extra::error::Error),
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),
}
