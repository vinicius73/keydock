use std::sync::Arc;

use keydock_support::Clock;

/// Per-request / per-operation context (auth, tenant), expanded later.
#[derive(Clone)]
pub struct RequestContext {
    pub clock: Arc<dyn Clock>,
}

impl RequestContext {
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self { clock }
    }
}
