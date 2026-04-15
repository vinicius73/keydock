use thiserror::Error;

/// Technical errors that are not domain, HTTP, or storage specific.
#[derive(Debug, Error)]
pub enum SupportError {
    #[error("invalid secret material")]
    InvalidSecret,
}
