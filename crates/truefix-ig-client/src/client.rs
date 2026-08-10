use reqwest::{Method, RequestBuilder, StatusCode};
use serde::{Serialize, de::DeserializeOwned};
use std::sync::RwLock;
use std::time::{Duration, Instant};
use url::Url;

use crate::{
    config::{AuthenticationVersion, ClientConfig, Credentials},
    error::{IgError, IgResult},
    streaming::IgStreamingClient,
    types::{
        AccountsResponse, CreatePositionRequest, CreateWorkingOrderRequest, DealConfirmation,
        DealReferenceResponse, HistoricalPricesQuery, HistoricalPricesResponse, LoginResponse,
        MarketDetails, MarketsResponse, OAuthToken, PositionsResponse, SwitchAccountResponse,
        UpdateWorkingOrderRequest, V3LoginResponse, WorkingOrdersResponse,
    },
};

#[derive(Debug, Clone)]
struct SessionTokens {
    cst: String,
    x_security_token: String,
    account_id: String,
    lightstreamer_endpoint: String,
}

#[derive(Debug, Clone)]
struct OAuthSession {
    access_token: String,
    refresh_token: String,
    account_id: String,
    lightstreamer_endpoint: String,
    expires_at: Instant,
}

#[derive(Debug, Clone)]
enum Session {
    V2(SessionTokens),
    V3(OAuthSession),
}

const TOKEN_REFRESH_THRESHOLD: Duration = Duration::from_secs(10);

/// IG REST trading client.
///
/// Session credentials are kept behind a lock so that one client can be shared among
/// concurrent read-only tasks after [`Self::login`] succeeds. Calls which change server state are
/// deliberately made once and are never automatically replayed.
#[derive(Debug)]
pub struct IgClient {
    http: reqwest::Client,
    config: ClientConfig,
    base_url: Url,
    session: RwLock<Option<Session>>,
}

impl IgClient {
    /// Constructs a client without contacting IG.
    pub fn new(config: ClientConfig) -> IgResult<Self> {
        let base_url = Url::parse(config.environment.rest_base()).map_err(|error| {
            IgError::InvalidConfiguration(format!("invalid REST base URL: {error}"))
        })?;
        let mut http = reqwest::Client::builder().timeout(config.timeout);
        if let Some(proxy) = &config.proxy {
            http = http.proxy(reqwest::Proxy::all(proxy).map_err(|error| {
                IgError::InvalidConfiguration(format!("invalid proxy URL: {error}"))
            })?);
        }
        Ok(Self {
            http: http.build()?,
            config,
            base_url,
            session: RwLock::new(None),
        })
    }

    /// Authenticates using the protocol selected in [`ClientConfig::authentication`].
    pub async fn login(&self) -> IgResult<LoginResponse> {
        match &self.config.authentication {
            AuthenticationVersion::V2 => self.login_v2().await,
            AuthenticationVersion::V3 { account_id } => self.login_v3(account_id).await,
        }
    }

    /// Authenticates with v2 and stores the CST/X-SECURITY-TOKEN pair.
    pub async fn login_v2(&self) -> IgResult<LoginResponse> {
        #[derive(Serialize)]
        struct LoginRequest<'a> {
            identifier: &'a str,
            password: &'a str,
        }

        let credentials = self.credentials()?;
        let response = self
            .request(Method::POST, "session", 2, false)?
            .json(&LoginRequest {
                identifier: credentials.identifier(),
                password: credentials.password(),
            })
            .send()
            .await?;
        let (tokens, login) = self.decode_login(response).await?;
        *self.session.write().map_err(|_| IgError::MissingSession)? = Some(Session::V2(tokens));
        Ok(login)
    }

    /// Authenticates with v3 OAuth and stores refreshable bearer credentials.
    pub async fn login_v3(&self, account_id: &str) -> IgResult<LoginResponse> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct LoginRequest<'a> {
            identifier: &'a str,
            password: &'a str,
            account_id: &'a str,
        }

        if account_id.is_empty() {
            return Err(IgError::InvalidConfiguration(
                "v3 authentication requires a non-empty account ID".to_owned(),
            ));
        }
        let credentials = self.credentials()?;
        let response = self
            .request(Method::POST, "session", 3, false)?
            .header("IG-ACCOUNT-ID", account_id)
            .json(&LoginRequest {
                identifier: credentials.identifier(),
                password: credentials.password(),
                account_id,
            })
            .send()
            .await?;
        let response = Self::ensure_success(response).await?;
        let login: V3LoginResponse = response.json().await?;
        let oauth = login.oauth_token.ok_or_else(|| {
            IgError::InvalidConfiguration(
                "IG v3 login response did not contain an OAuth token".to_owned(),
            )
        })?;
        let selected_account = login
            .account_id
            .clone()
            .unwrap_or_else(|| account_id.to_owned());
        let result = LoginResponse {
            current_account_id: login.current_account_id.or(login.account_id),
            lightstreamer_endpoint: login.lightstreamer_endpoint,
            client_id: login.client_id,
            currency_iso_code: login.currency_iso_code,
            dealing_enabled: login.dealing_enabled,
        };
        *self.session.write().map_err(|_| IgError::MissingSession)? =
            Some(Session::V3(oauth_session(
                oauth,
                selected_account,
                result.lightstreamer_endpoint.clone(),
            )));
        Ok(result)
    }

    /// Deletes the current session and clears local tokens only after IG accepts the request.
    pub async fn logout(&self) -> IgResult<()> {
        self.refresh_oauth_if_needed().await?;
        let response = self
            .request(Method::DELETE, "session", 1, true)?
            .send()
            .await?;
        Self::ensure_success(response).await?;
        *self.session.write().map_err(|_| IgError::MissingSession)? = None;
        Ok(())
    }

    /// Returns the accounts available to the authenticated client.
    pub async fn accounts(&self) -> IgResult<AccountsResponse> {
        self.get("accounts", 1).await
    }

    /// Switches the active V2 account without changing the user's default.
    ///
    /// IG rotates the X-SECURITY-TOKEN when the active account changes. The
    /// returned header is committed atomically with the local account ID so
    /// subsequent REST and Lightstreamer calls cannot retain the old account
    /// token.
    pub async fn switch_account(&self, account_id: &str) -> IgResult<SwitchAccountResponse> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct SwitchRequest<'a> {
            account_id: &'a str,
            default_account: bool,
        }

        if account_id.trim().is_empty() {
            return Err(IgError::InvalidConfiguration(
                "account switch requires a non-empty account ID".to_owned(),
            ));
        }
        let response = self
            .request(Method::PUT, "session", 1, true)?
            .json(&SwitchRequest {
                account_id,
                default_account: false,
            })
            .send()
            .await?;
        let response = Self::ensure_success(response).await?;
        let next_token = response
            .headers()
            .get("X-SECURITY-TOKEN")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let result: SwitchAccountResponse = response.json().await?;
        let mut session = self.session.write().map_err(|_| IgError::MissingSession)?;
        let Some(Session::V2(tokens)) = session.as_mut() else {
            return Err(IgError::InvalidConfiguration(
                "account switching requires a V2 CST/XST session".to_owned(),
            ));
        };
        tokens.account_id = account_id.to_owned();
        if let Some(token) = next_token {
            tokens.x_security_token = token;
        }
        Ok(result)
    }

    /// Returns all open positions for the active account.
    pub async fn positions(&self) -> IgResult<PositionsResponse> {
        self.get("positions", 2).await
    }

    /// Returns market metadata and a latest price snapshot for one IG epic.
    pub async fn market(&self, epic: &str) -> IgResult<MarketDetails> {
        self.get(&format!("markets/{}", encode_path_segment(epic)), 3)
            .await
    }

    /// Searches the market catalogue using IG's `/markets` endpoint.
    pub async fn search_markets(&self, query: &str) -> IgResult<MarketsResponse> {
        self.refresh_oauth_if_needed().await?;
        let mut url = self.url("markets")?;
        url.query_pairs_mut().append_pair("searchTerm", query);
        self.send_json(self.request_url(Method::GET, url, 1, true)?)
            .await
    }

    /// Returns historical prices for an epic at the requested IG resolution.
    pub async fn historical_prices(
        &self,
        epic: &str,
        query: HistoricalPricesQuery<'_>,
    ) -> IgResult<HistoricalPricesResponse> {
        self.refresh_oauth_if_needed().await?;
        let mut url = self.url(&format!("prices/{}", encode_path_segment(epic)))?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("resolution", query.resolution);
            if let Some(value) = query.from {
                pairs.append_pair("from", value);
            }
            if let Some(value) = query.to {
                pairs.append_pair("to", value);
            }
            if let Some(value) = query.max {
                pairs.append_pair("max", &value.to_string());
            }
        }
        self.send_json(self.request_url(Method::GET, url, 3, true)?)
            .await
    }

    /// Creates a position. This operation is never automatically retried.
    pub async fn create_position(
        &self,
        request: &CreatePositionRequest,
    ) -> IgResult<DealReferenceResponse> {
        self.refresh_oauth_if_needed().await?;
        let response = self
            .request(Method::POST, "positions/otc", 2, true)?
            .json(request)
            .send()
            .await?;
        Self::decode_json(response).await
    }

    /// Resolves a submission acknowledgement into IG's authoritative deal
    /// confirmation. Callers must not treat the initial deal reference as an
    /// accepted order.
    pub async fn deal_confirmation(&self, deal_reference: &str) -> IgResult<DealConfirmation> {
        self.get(
            &format!("confirms/{}", encode_path_segment(deal_reference)),
            1,
        )
        .await
    }

    /// Returns every working order for the active account.
    pub async fn working_orders(&self) -> IgResult<WorkingOrdersResponse> {
        self.get("workingorders", 2).await
    }

    /// Creates a working order. Resolve the returned reference with
    /// [`Self::deal_confirmation`] before treating it as accepted.
    pub async fn create_working_order(
        &self,
        request: &CreateWorkingOrderRequest,
    ) -> IgResult<DealReferenceResponse> {
        self.mutate_json(Method::POST, "workingorders/otc", 2, request)
            .await
    }

    /// Updates an existing working order. Writes are never automatically replayed.
    pub async fn update_working_order(
        &self,
        deal_id: &str,
        request: &UpdateWorkingOrderRequest,
    ) -> IgResult<DealReferenceResponse> {
        self.mutate_json(
            Method::PUT,
            &format!("workingorders/otc/{}", encode_path_segment(deal_id)),
            2,
            request,
        )
        .await
    }

    /// Deletes an existing working order. Writes are never automatically replayed.
    pub async fn delete_working_order(&self, deal_id: &str) -> IgResult<DealReferenceResponse> {
        self.refresh_oauth_if_needed().await?;
        let response = self
            .request(
                Method::DELETE,
                &format!("workingorders/otc/{}", encode_path_segment(deal_id)),
                2,
                true,
            )?
            .send()
            .await?;
        Self::decode_json(response).await
    }

    /// Opens IG's Lightstreamer endpoint using credentials issued by v2 login.
    pub async fn connect_streaming(&self) -> IgResult<IgStreamingClient> {
        enum StreamingCredentials {
            Ready(SessionTokens),
            Fetch {
                account_id: String,
                endpoint: String,
            },
        }
        let credentials = {
            let session = self.session.read().map_err(|_| IgError::MissingSession)?;
            match session.as_ref() {
                Some(Session::V2(tokens)) => StreamingCredentials::Ready(tokens.clone()),
                Some(Session::V3(oauth)) => StreamingCredentials::Fetch {
                    account_id: oauth.account_id.clone(),
                    endpoint: oauth.lightstreamer_endpoint.clone(),
                },
                None => return Err(IgError::MissingSession),
            }
        };
        let tokens = match credentials {
            StreamingCredentials::Ready(tokens) => tokens,
            StreamingCredentials::Fetch {
                account_id,
                endpoint,
            } => self.fetch_streaming_tokens(account_id, endpoint).await?,
        };
        IgStreamingClient::connect(
            &tokens.lightstreamer_endpoint,
            tokens.account_id,
            tokens.cst,
            tokens.x_security_token,
        )
        .await
    }

    async fn fetch_streaming_tokens(
        &self,
        account_id: String,
        lightstreamer_endpoint: String,
    ) -> IgResult<SessionTokens> {
        self.refresh_oauth_if_needed().await?;
        let mut url = self.url("session")?;
        url.query_pairs_mut()
            .append_pair("fetchSessionTokens", "true");
        let response = self.request_url(Method::GET, url, 1, true)?.send().await?;
        let response = Self::ensure_success(response).await?;
        let cst = response
            .headers()
            .get("CST")
            .and_then(|value| value.to_str().ok())
            .ok_or(IgError::MissingToken("CST"))?
            .to_owned();
        let x_security_token = response
            .headers()
            .get("X-SECURITY-TOKEN")
            .and_then(|value| value.to_str().ok())
            .ok_or(IgError::MissingToken("X-SECURITY-TOKEN"))?
            .to_owned();
        Ok(SessionTokens {
            cst,
            x_security_token,
            account_id,
            lightstreamer_endpoint,
        })
    }

    async fn mutate_json<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        version: u8,
        body: &B,
    ) -> IgResult<T> {
        self.refresh_oauth_if_needed().await?;
        let response = self
            .request(method, path, version, true)?
            .json(body)
            .send()
            .await?;
        Self::decode_json(response).await
    }

    fn credentials(&self) -> IgResult<&Credentials> {
        self.config
            .credentials
            .as_ref()
            .ok_or(IgError::MissingCredentials)
    }

    fn url(&self, path: &str) -> IgResult<Url> {
        let mut url = self.base_url.clone();
        url.set_path(&format!(
            "{}/{}",
            self.base_url.path().trim_end_matches('/'),
            path.trim_start_matches('/')
        ));
        url.set_query(None);
        Ok(url)
    }

    fn request(
        &self,
        method: Method,
        path: &str,
        version: u8,
        include_session: bool,
    ) -> IgResult<RequestBuilder> {
        self.request_url(method, self.url(path)?, version, include_session)
    }

    fn request_url(
        &self,
        method: Method,
        url: Url,
        version: u8,
        include_session: bool,
    ) -> IgResult<RequestBuilder> {
        let credentials = self.credentials()?;
        let mut request = self
            .http
            .request(method, url)
            .header("X-IG-API-KEY", credentials.api_key())
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("Version", version.to_string());
        if include_session {
            let session = self.session.read().map_err(|_| IgError::MissingSession)?;
            let session = session.as_ref().ok_or(IgError::MissingSession)?;
            request = match session {
                Session::V2(tokens) => request
                    .header("CST", &tokens.cst)
                    .header("X-SECURITY-TOKEN", &tokens.x_security_token),
                Session::V3(oauth) => request
                    .header("Authorization", format!("Bearer {}", oauth.access_token))
                    .header("IG-ACCOUNT-ID", &oauth.account_id),
            };
        }
        Ok(request)
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, version: u8) -> IgResult<T> {
        self.refresh_oauth_if_needed().await?;
        self.send_json(self.request(Method::GET, path, version, true)?)
            .await
    }

    async fn send_json<T: DeserializeOwned>(&self, request: RequestBuilder) -> IgResult<T> {
        Self::decode_json(request.send().await?).await
    }

    async fn decode_login(
        &self,
        response: reqwest::Response,
    ) -> IgResult<(SessionTokens, LoginResponse)> {
        let response = Self::ensure_success(response).await?;
        let cst = response
            .headers()
            .get("CST")
            .and_then(|value| value.to_str().ok())
            .ok_or(IgError::MissingToken("CST"))?
            .to_owned();
        let x_security_token = response
            .headers()
            .get("X-SECURITY-TOKEN")
            .and_then(|value| value.to_str().ok())
            .ok_or(IgError::MissingToken("X-SECURITY-TOKEN"))?
            .to_owned();
        let login: LoginResponse = response.json().await?;
        let account_id = login.current_account_id.clone().ok_or_else(|| {
            IgError::InvalidConfiguration(
                "IG v2 login response did not contain currentAccountId".to_owned(),
            )
        })?;
        let lightstreamer_endpoint = login.lightstreamer_endpoint.clone();
        Ok((
            SessionTokens {
                cst,
                x_security_token,
                account_id,
                lightstreamer_endpoint,
            },
            login,
        ))
    }

    async fn refresh_oauth_if_needed(&self) -> IgResult<()> {
        let refresh_token = {
            let session = self.session.read().map_err(|_| IgError::MissingSession)?;
            match session.as_ref() {
                Some(Session::V3(oauth))
                    if oauth.expires_at.saturating_duration_since(Instant::now())
                        <= TOKEN_REFRESH_THRESHOLD =>
                {
                    Some(oauth.refresh_token.clone())
                }
                _ => None,
            }
        };
        let Some(refresh_token) = refresh_token else {
            return Ok(());
        };

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct RefreshTokenRequest<'a> {
            refresh_token: &'a str,
        }

        let response = self
            .request(Method::POST, "session/refresh-token", 1, false)?
            .json(&RefreshTokenRequest {
                refresh_token: &refresh_token,
            })
            .send()
            .await?;
        let oauth: OAuthToken = Self::decode_json(response).await?;
        let mut session = self.session.write().map_err(|_| IgError::MissingSession)?;
        let Some(Session::V3(current)) = session.as_mut() else {
            return Ok(());
        };
        current.access_token = oauth.access_token;
        current.refresh_token = oauth.refresh_token;
        current.expires_at = expiry_from(oauth.expires_in);
        Ok(())
    }

    async fn decode_json<T: DeserializeOwned>(response: reqwest::Response) -> IgResult<T> {
        Self::ensure_success(response)
            .await?
            .json()
            .await
            .map_err(IgError::from)
    }

    async fn ensure_success(response: reqwest::Response) -> IgResult<reqwest::Response> {
        if response.status().is_success() {
            return Ok(response);
        }
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(exchange_error(status, &body))
    }
}

fn exchange_error(status: StatusCode, body: &str) -> IgError {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ErrorBody {
        error_code: Option<String>,
        message: Option<String>,
    }
    let parsed = serde_json::from_str::<ErrorBody>(body).ok();
    let code = parsed
        .as_ref()
        .and_then(|value| value.error_code.clone())
        .unwrap_or_else(|| "unknown".to_owned());
    let message = parsed
        .and_then(|value| value.message)
        .unwrap_or_else(|| body.to_owned());
    IgError::Exchange {
        status: status.as_u16(),
        code,
        message,
    }
}

fn oauth_session(
    token: OAuthToken,
    account_id: String,
    lightstreamer_endpoint: String,
) -> OAuthSession {
    OAuthSession {
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        account_id,
        lightstreamer_endpoint,
        expires_at: expiry_from(token.expires_in),
    }
}

fn expiry_from(expires_in: u64) -> Instant {
    Instant::now() + Duration::from_secs(expires_in.max(1))
}

fn encode_path_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Environment;

    #[test]
    fn custom_base_preserves_gateway_path() {
        let config = ClientConfig {
            environment: Environment::Custom {
                rest_base: "http://127.0.0.1:8080/gateway/deal".to_owned(),
            },
            ..ClientConfig::default()
        };
        let client = IgClient::new(config).unwrap();
        assert_eq!(
            client.url("session").unwrap().as_str(),
            "http://127.0.0.1:8080/gateway/deal/session"
        );
    }

    #[test]
    fn epics_are_encoded_as_one_path_segment() {
        assert_eq!(encode_path_segment("CS.D/EUR USD"), "CS.D%2FEUR%20USD");
    }

    #[test]
    fn encoded_epic_is_not_encoded_twice_in_a_url() {
        let config = ClientConfig {
            environment: Environment::Custom {
                rest_base: "http://127.0.0.1:8080/gateway/deal".to_owned(),
            },
            ..ClientConfig::default()
        };
        let client = IgClient::new(config).unwrap();
        assert_eq!(
            client.url("markets/CS.D%2FEUR").unwrap().path(),
            "/gateway/deal/markets/CS.D%2FEUR"
        );
    }
}
