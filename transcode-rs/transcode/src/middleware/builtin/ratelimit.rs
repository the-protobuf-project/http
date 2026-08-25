//! Rate limiting.

use crate::error::{Code, Detail, Error, QuotaFailure, QuotaViolation, Result, RetryInfo};
use crate::middleware::{Interceptor, RouteCx};
use std::sync::Arc;
use std::time::Duration;

/// Decides whether a call is within its quota.
///
/// The transcoder does not implement the counting: a real limit is shared across
/// replicas and belongs in Redis or a sidecar, and a per-process token bucket
/// would quietly permit N times the configured rate.
pub trait Limiter: Send + Sync + 'static {
    /// Whether this call may proceed.
    ///
    /// `key` identifies the subject — a caller id, an IP, a project.
    ///
    /// # Errors
    ///
    /// The delay before retrying, which becomes `429` plus `Retry-After`.
    fn allow(&self, key: &str, method: &str) -> std::result::Result<(), Duration>;
}

/// Rejects a call that is over quota.
///
/// The `429` carries a `QuotaFailure` naming the subject and a `RetryInfo` the
/// error model projects to `Retry-After`, so a client knows both why it was
/// refused and when to come back.
#[derive(Clone)]
pub struct RateLimit {
    limiter: Arc<dyn Limiter>,
    domain: &'static str,
}

impl RateLimit {
    /// Builds the interceptor.
    #[must_use]
    pub fn new(limiter: impl Limiter, domain: &'static str) -> Self {
        Self {
            limiter: Arc::new(limiter),
            domain,
        }
    }

    /// The subject a limit applies to.
    ///
    /// The authenticated caller when there is one, otherwise the resolved
    /// client address, otherwise the method. Preferring identity over address
    /// matters: NAT puts many callers behind one IP, and limiting by address
    /// alone punishes all of them for one.
    fn key(cx: &RouteCx<'_>) -> String {
        if let Some(identity) = cx.extensions.get::<super::Identity>() {
            return format!("sub:{}", identity.subject);
        }
        if let Some(ip) = cx.extensions.get::<super::realip::ClientIp>() {
            return format!("ip:{}", ip.0);
        }
        format!("method:{}", cx.method)
    }
}

impl std::fmt::Debug for RateLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RateLimit")
            .field("domain", &self.domain)
            .finish()
    }
}

impl Interceptor for RateLimit {
    fn name(&self) -> &'static str {
        "rate-limit"
    }

    fn on_route(&self, cx: &mut RouteCx<'_>) -> Result<()> {
        let key = Self::key(cx);
        let Err(retry_after) = self.limiter.allow(&key, cx.method) else {
            return Ok(());
        };

        Err(Box::new(
            Error::new(Code::ResourceExhausted, "Quota exceeded for this caller.")
                .with_error_info(
                    "RATE_LIMIT_EXCEEDED",
                    self.domain,
                    [("method".into(), cx.method.to_string())],
                )
                .with_detail(Detail::QuotaFailure(QuotaFailure {
                    violations: vec![QuotaViolation {
                        subject: key,
                        description: format!("Too many requests to {}.", cx.method),
                    }],
                }))
                .with_detail(Detail::RetryInfo(RetryInfo {
                    retry_delay: Some(prost_types::Duration {
                        seconds: i64::try_from(retry_after.as_secs()).unwrap_or(i64::MAX),
                        nanos: i32::try_from(retry_after.subsec_nanos()).unwrap_or(0),
                    }),
                })),
        ))
    }
}
