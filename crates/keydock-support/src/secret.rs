use secrecy::{ExposeSecret, SecretString};

/// Wraps [`SecretString`] to avoid accidental logging: `Debug` stays redacted.
#[derive(Clone)]
pub struct RedactedSecret(SecretString);

impl RedactedSecret {
    pub fn new(value: impl Into<String>) -> Self {
        let s: String = value.into();
        Self(SecretString::from(Box::from(s)))
    }

    pub fn expose(&self) -> &str {
        self.0.expose_secret()
    }
}

impl std::fmt::Debug for RedactedSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_is_redacted() {
        let s = RedactedSecret::new("super-secret");
        let dbg = format!("{s:?}");
        assert!(!dbg.contains("super-secret"));
        assert!(dbg.contains("REDACTED"));
    }
}
