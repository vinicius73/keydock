use time::OffsetDateTime;

/// Instants for tests and production (avoid direct `OffsetDateTime::now_utc` in use cases when injectable).
pub trait Clock: Send + Sync {
    fn now_utc(&self) -> OffsetDateTime;
}

/// System UTC clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_utc(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_clock_is_monotonic() {
        let c = SystemClock;
        let a = c.now_utc();
        let b = c.now_utc();
        assert!(b >= a);
    }
}
