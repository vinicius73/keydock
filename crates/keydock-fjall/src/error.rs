use keydock_usecase::UseCaseError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FjallError {
    #[error(transparent)]
    Fjall(#[from] fjall::Error),

    #[error("storage adapter error: {0}")]
    Adapter(String),
}

impl From<FjallError> for UseCaseError {
    fn from(e: FjallError) -> Self {
        Self::Storage(e.to_string())
    }
}

impl From<crate::codec::CodecError> for UseCaseError {
    fn from(e: crate::codec::CodecError) -> Self {
        Self::Storage(e.to_string())
    }
}
