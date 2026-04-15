use std::sync::Arc;

use keydock_support::Clock;

use crate::auth::ResolvedIdentity;

/// Per-request / per-operation context (auth, tenant).
#[derive(Clone)]
pub struct RequestContext {
    pub clock: Arc<dyn Clock>,
    pub identity: ResolvedIdentity,
}

impl RequestContext {
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            clock,
            identity: ResolvedIdentity::Anonymous,
        }
    }

    pub fn with_identity(clock: Arc<dyn Clock>, identity: ResolvedIdentity) -> Self {
        Self { clock, identity }
    }
}
