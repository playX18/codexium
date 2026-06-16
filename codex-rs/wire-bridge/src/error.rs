use thiserror::Error;

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("transform error: {0}")]
    TransformError(String),
}
