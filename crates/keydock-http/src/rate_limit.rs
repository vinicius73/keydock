use tracing::instrument;

#[derive(Debug, Clone)]
pub struct RateLimitSettings {
    pub enabled: bool,
    pub requests_per_hour: u64,
}

impl Default for RateLimitSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            requests_per_hour: 1000,
        }
    }
}

/// Initializes the global `lazy-limit` limiter for this process.
///
/// `axum-governor` reads rate-limit rules from `lazy-limit`, so this must run
/// before wiring `GovernorLayer` when `enabled = true`.
#[instrument(skip_all, name = "rate_limit::init_rate_limiter", fields(enabled = settings.enabled, requests_per_hour = settings.requests_per_hour))]
pub async fn init_rate_limiter(settings: &RateLimitSettings) {
    if !settings.enabled {
        return;
    }

    let tokens = match u32::try_from(settings.requests_per_hour) {
        Ok(v) => v,
        Err(_) => {
            tracing::warn!(
                requests_per_hour = settings.requests_per_hour,
                "requests_per_hour exceeds u32::MAX; capping to u32::MAX"
            );
            u32::MAX
        }
    };
    let rule = lazy_limit::RuleConfig::new(lazy_limit::Duration::hours(1), tokens);
    let limiter_config = lazy_limit::LimiterConfig::new(rule);
    lazy_limit::initialize_limiter(limiter_config).await;
}
