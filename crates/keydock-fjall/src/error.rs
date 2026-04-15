use thiserror::Error;

#[derive(Debug, Error)]
pub enum FjallError {
    #[error(transparent)]
    Fjall(#[from] fjall::Error),

    #[error("storage adapter error: {0}")]
    Adapter(String),
}
