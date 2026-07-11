use std::sync::Arc;

use time::OffsetDateTime;

use crate::{
    auth::{Clock, OffsetClock, SystemClock},
    config::ClientConfig,
    error::OkxResult,
    inventory::{AuthClass, BASELINE_OPERATION_MANIFEST, ReplayClass},
    limiter::RateLimiter,
    request::CanonicalRequest,
    response::{OkxResponse, decode_envelope_with_metadata},
    services::{
        account::AccountService, finance::FinanceService, funding::FundingService,
        market::MarketService, professional::ProfessionalService, public_data::PublicDataService,
        strategy::StrategyService, subaccount::SubaccountService, trade::TradeService,
    },
    transport::http::HttpTransport,
};

/// Native OKX client with one immutable identity and environment.
pub struct OkxClient {
    config: Arc<ClientConfig>,
    http: HttpTransport,
    limiter: RateLimiter,
}

impl OkxClient {
    /// Creates a client using the system clock.
    pub fn new(config: ClientConfig) -> OkxResult<Self> {
        Self::with_clock(config, Arc::new(SystemClock))
    }
    /// Creates a client with a testable signing clock.
    pub fn with_clock(config: ClientConfig, clock: Arc<dyn Clock>) -> OkxResult<Self> {
        let config = Arc::new(config);
        let clock = Arc::new(OffsetClock::new(clock, config.clock_offset));
        let http = HttpTransport::new(Arc::clone(&config), clock)?;
        Ok(Self {
            config,
            http,
            limiter: RateLimiter::default(),
        })
    }
    /// Returns the immutable configuration bound to this client.
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }
    pub fn account(&self) -> AccountService<'_> {
        AccountService(self)
    }
    pub fn market(&self) -> MarketService<'_> {
        MarketService(self)
    }
    /// Accesses public reference data without signing requests.
    pub fn public_data(&self) -> PublicDataService<'_> {
        PublicDataService(self)
    }
    pub fn trade(&self) -> TradeService<'_> {
        TradeService(self)
    }
    /// Returns funding-account and asset-movement operations.
    pub fn funding(&self) -> FundingService<'_> {
        FundingService(self)
    }
    /// Returns sub-account administrative operations.
    pub fn subaccounts(&self) -> SubaccountService<'_> {
        SubaccountService(self)
    }
    /// Returns savings, staking, loan, and investment operations.
    pub fn finance(&self) -> FinanceService<'_> {
        FinanceService(self)
    }
    /// Returns grid, recurring-buy, and copy-trading operations.
    pub fn strategy(&self) -> StrategyService<'_> {
        StrategyService(self)
    }
    /// Returns block, spread, broker, conversion, and status operations.
    pub fn professional(&self) -> ProfessionalService<'_> {
        ProfessionalService(self)
    }

    /// Measures the difference between the local clock and OKX server time.
    ///
    /// The returned value is `server_time - local_time_at_response_midpoint` and can be passed
    /// directly to [`ClientConfig::with_clock_offset`].  Taking local timestamps immediately
    /// before and after the request compensates for approximately half of the request round-trip
    /// time. Callers should repeat the measurement and prefer a result with a small round-trip
    /// time when their network path is asymmetric.
    pub async fn measure_server_time_offset(&self) -> OkxResult<time::Duration> {
        #[derive(serde::Deserialize)]
        struct ServerTime {
            ts: String,
        }

        let started_at = OffsetDateTime::now_utc();
        let response: Vec<ServerTime> = self
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/public/time",
                std::collections::BTreeMap::new(),
                None::<&serde_json::Value>,
                crate::request::RetrySafety::ReadOnly,
                false,
            )?)
            .await?;
        let received_at = OffsetDateTime::now_utc();
        let server_time = response
            .into_iter()
            .next()
            .ok_or_else(|| {
                crate::error::OkxError::InvalidConfiguration(
                    "OKX server-time response contained no timestamp".to_owned(),
                )
            })?
            .ts
            .parse::<i128>()
            .map_err(|error| {
                crate::error::OkxError::InvalidConfiguration(format!(
                    "OKX server-time response contained an invalid millisecond timestamp: {error}"
                ))
            })?;
        let server_time = OffsetDateTime::from_unix_timestamp_nanos(server_time * 1_000_000)
            .map_err(|error| {
                crate::error::OkxError::InvalidConfiguration(format!(
                    "OKX server-time timestamp is out of range: {error}"
                ))
            })?;
        let midpoint = started_at + (received_at - started_at) / 2;
        Ok(server_time - midpoint)
    }

    /// Executes one REST operation recorded in the SDK's audited baseline manifest.
    ///
    /// This is intended for tools that need generic access to the complete supported REST
    /// surface. `domain` and `operation` must match a manifest entry exactly; arbitrary paths
    /// cannot be submitted through this method. Typed service methods remain preferable for
    /// application code.
    pub async fn execute_baseline_operation(
        &self,
        domain: &str,
        operation: &str,
        query: std::collections::BTreeMap<String, String>,
        body: Option<&serde_json::Value>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        Ok(self
            .execute_baseline_operation_with_metadata(domain, operation, query, body)
            .await?
            .data)
    }

    /// Executes a manifest operation and retains OKX pagination and request metadata.
    pub async fn execute_baseline_operation_with_metadata(
        &self,
        domain: &str,
        operation: &str,
        query: std::collections::BTreeMap<String, String>,
        body: Option<&serde_json::Value>,
    ) -> OkxResult<OkxResponse<serde_json::Value>> {
        let entry = BASELINE_OPERATION_MANIFEST
            .iter()
            .find(|entry| entry.domain == domain && entry.operation == operation)
            .ok_or_else(|| {
                crate::error::OkxError::InvalidConfiguration(format!(
                    "unknown OKX baseline operation `{domain}/{operation}`"
                ))
            })?;
        let method = match entry.method {
            "GET" => reqwest::Method::GET,
            "POST" => reqwest::Method::POST,
            _ => {
                return Err(crate::error::OkxError::InvalidConfiguration(format!(
                    "unsupported method in operation manifest: {}",
                    entry.method
                )));
            }
        };
        if entry.method == "GET" && body.is_some() {
            return Err(crate::error::OkxError::InvalidConfiguration(
                "GET operations do not accept a JSON body".to_owned(),
            ));
        }
        if entry.method == "POST" && body.is_none() && !baseline_post_accepts_empty_body(entry) {
            return Err(crate::error::OkxError::InvalidConfiguration(
                "POST operations require a JSON body".to_owned(),
            ));
        }
        // The pinned python-okx baseline invokes these endpoints via
        // `_request_without_params(POST, ...)`.  Its request helper sends an
        // empty JSON object, rather than an absent body; preserve those exact
        // wire semantics for generic callers too.
        let empty_body = serde_json::json!({});
        let body = if body.is_none() && baseline_post_accepts_empty_body(entry) {
            Some(&empty_body)
        } else {
            body
        };
        self.execute_with_metadata(CanonicalRequest::new(
            method,
            entry.path,
            query,
            body,
            match entry.replay {
                ReplayClass::ReadOnly => crate::request::RetrySafety::ReadOnly,
                ReplayClass::NeverReplay => crate::request::RetrySafety::NeverReplay,
            },
            matches!(entry.auth, AuthClass::Private),
        )?)
        .await
    }

    pub(crate) async fn execute<T: serde::de::DeserializeOwned>(
        &self,
        request: CanonicalRequest,
    ) -> OkxResult<Vec<T>> {
        Ok(self.execute_with_metadata(request).await?.data)
    }

    async fn execute_with_metadata<T: serde::de::DeserializeOwned>(
        &self,
        request: CanonicalRequest,
    ) -> OkxResult<OkxResponse<T>> {
        self.limiter.reserve().await;
        let safety = request.retry_safety;
        let response = match self.http.execute(request.clone()).await {
            Ok(response) => response,
            Err(error) if RateLimiter::may_retry(safety, &error) => {
                if let crate::error::OkxError::RateLimited { retry_after } = &error {
                    self.limiter
                        .throttle_for(retry_after.unwrap_or(std::time::Duration::from_millis(100)))
                        .await;
                }
                self.limiter.reserve().await;
                self.http.execute(request).await?
            }
            Err(error) => return Err(error),
        };
        decode_envelope_with_metadata(&response.body, response.metadata)
    }
}

fn baseline_post_accepts_empty_body(entry: &crate::inventory::BaselineOperation) -> bool {
    matches!(
        (entry.domain, entry.operation),
        ("account", "activate_option") | ("block_trading", "reset_mmp")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn baseline_operation_rejects_unknown_and_malformed_requests_before_transport() {
        let client = OkxClient::new(ClientConfig::default()).unwrap();
        let query = std::collections::BTreeMap::new();

        assert!(matches!(
            client
                .execute_baseline_operation("unknown", "operation", query.clone(), None)
                .await,
            Err(crate::error::OkxError::InvalidConfiguration(_))
        ));
        assert!(matches!(
            client
                .execute_baseline_operation(
                    "market_data",
                    "get_ticker",
                    query.clone(),
                    Some(&serde_json::json!({}))
                )
                .await,
            Err(crate::error::OkxError::InvalidConfiguration(_))
        ));
        assert!(matches!(
            client
                .execute_baseline_operation("trade", "place_order", query, None)
                .await,
            Err(crate::error::OkxError::InvalidConfiguration(_))
        ));
    }
}
