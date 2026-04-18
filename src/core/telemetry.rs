//! Outbound usage telemetry has been retired.
//!
//! Local tracking, gain analytics, worldview packets, and packet telemetry stay
//! in place, but the binary no longer sends anonymous usage pings to any remote
//! endpoint. Keep this module as a stable no-op so existing call sites and
//! future integrations cannot accidentally re-enable network telemetry.

/// Outbound telemetry is intentionally disabled.
#[allow(dead_code)]
pub fn maybe_ping() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_maybe_ping_is_noop() {
        maybe_ping();
    }
}
