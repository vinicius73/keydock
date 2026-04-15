use thiserror::Error;

#[derive(Debug, Error)]
pub enum UseCaseError {
    #[error("not implemented")]
    NotImplemented,

    #[error(transparent)]
    Domain(#[from] keydock_domain::DomainError),
}
