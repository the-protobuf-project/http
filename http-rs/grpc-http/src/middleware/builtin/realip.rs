//! Recovering the client's address from proxy headers.

use crate::error::Result;
use crate::middleware::{Interceptor, RouteCx};
use std::net::IpAddr;

/// The caller's address, as resolved from proxy headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientIp(pub IpAddr);

/// Resolves the client address behind a proxy.
///
/// `X-Forwarded-For` accumulates left to right, so the leftmost entry is the
/// original client — and is also entirely client-controlled. [`RealIp::trusted_hops`]
/// says how many proxies at the *right* end are yours; the address is taken
/// from just left of them, and anything further left is ignored.
///
/// Getting this wrong is how IP rate limits and IP allowlists get bypassed, so
/// the default trusts nothing and falls back to the transport peer.
#[derive(Debug, Clone, Copy)]
pub struct RealIp {
    trusted_hops: usize,
}

impl RealIp {
    /// Trusts no proxy: the transport peer is the client.
    #[must_use]
    pub const fn direct() -> Self {
        Self { trusted_hops: 0 }
    }

    /// Trusts `hops` proxies closest to this server.
    ///
    /// One load balancer in front means `hops = 1`.
    #[must_use]
    pub const fn trusted_hops(hops: usize) -> Self {
        Self { trusted_hops: hops }
    }

    /// Resolves the client address for a request.
    #[must_use]
    pub fn resolve(&self, cx: &RouteCx<'_>) -> Option<IpAddr> {
        if self.trusted_hops == 0 {
            return cx.peer.map(|addr| addr.ip());
        }

        let forwarded: Vec<IpAddr> = cx
            .headers
            .get_all("x-forwarded-for")
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(','))
            .filter_map(|entry| entry.trim().parse().ok())
            .collect();

        // Count back past the proxies we trust. If the header is shorter than
        // that, it was not written by our own chain, so it is not trusted.
        forwarded
            .len()
            .checked_sub(self.trusted_hops)
            .and_then(|index| index.checked_sub(1))
            .and_then(|index| forwarded.get(index).copied())
            .or_else(|| cx.peer.map(|addr| addr.ip()))
    }
}

impl Interceptor for RealIp {
    fn name(&self) -> &'static str {
        "real-ip"
    }

    /// Publishes the resolved address for later interceptors, and forwards it
    /// to the service so the backend logs the caller rather than the proxy.
    fn on_route(&self, cx: &mut RouteCx<'_>) -> Result<()> {
        if let Some(ip) = self.resolve(cx) {
            cx.extensions.insert(ClientIp(ip));
            cx.metadata.append("x-forwarded-for", ip.to_string());
        }
        Ok(())
    }
}
