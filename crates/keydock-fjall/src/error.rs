use keydock_usecase::UseCaseError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FjallError {
    #[error(transparent)]
    Fjall(#[from] fjall::Error),

    /// Adapter plumbing failure (layout/keyspace mismatch, poisoned mutex)
    /// — everything that is not a backend call nor an entry codec error.
    #[error("storage adapter error: {0}")]
    Adapter(String),

    /// Entry (de)serialization failure. Distinct from [`FjallError::Adapter`]
    /// so the `storage_errors_total{kind="codec_entry"}` label is derived
    /// from the variant instead of string-matching messages.
    #[error("storage codec error: {0}")]
    Codec(String),
}

impl FjallError {
    /// Maps the variant to the stable Prometheus `kind` label.
    fn kind_label(&self) -> &'static str {
        match self {
            Self::Fjall(_) => "backend",
            Self::Adapter(_) => "adapter",
            Self::Codec(_) => "codec_entry",
        }
    }
}

fn record_storage_error(kind: &'static str) {
    metrics::counter!("storage_errors_total", "kind" => kind).increment(1);
}

// Single chokepoint: every storage failure reaches `UseCaseError` through one
// of these two `From` impls, so incrementing here guarantees the counter
// tracks *all* storage errors without scattering `metrics::counter!` calls
// across the adapter. Keep the label set in sync with `kind_label()` and
// `describe_all()` (crate `keydock-http`).
impl From<FjallError> for UseCaseError {
    fn from(e: FjallError) -> Self {
        record_storage_error(e.kind_label());
        Self::Storage(e.to_string())
    }
}

impl From<crate::codec::CodecError> for UseCaseError {
    fn from(e: crate::codec::CodecError) -> Self {
        record_storage_error("codec_policy");
        Self::Storage(e.to_string())
    }
}
