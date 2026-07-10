use std::sync::Arc;

use crate::{
    auth::{Clock, SystemClock},
    config::ClientConfig,
    error::OkxResult,
    limiter::RateLimiter,
    request::CanonicalRequest,
    response::decode_envelope,
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
    pub(crate) async fn execute<T: serde::de::DeserializeOwned>(
        &self,
        request: CanonicalRequest,
    ) -> OkxResult<Vec<T>> {
        self.limiter.reserve().await;
        decode_envelope(&self.http.execute(request).await?)
    }
}
