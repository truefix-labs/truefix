use std::sync::Arc;

use crate::{
    auth::{Clock, private_headers, sign_rest},
    config::{ClientConfig, Environment},
    error::{OkxError, OkxResult},
    request::CanonicalRequest,
    response::ResponseMetadata,
};

/// Exact HTTP response bytes plus metadata that is not present in the OKX JSON envelope.
pub(crate) struct HttpResponse {
    pub body: Vec<u8>,
    pub metadata: ResponseMetadata,
}

/// Reusable HTTP transport with the client configuration and signing clock.
pub struct HttpTransport {
    client: reqwest::Client,
    config: Arc<ClientConfig>,
    clock: Arc<dyn Clock>,
}

impl HttpTransport {
    pub fn new(config: Arc<ClientConfig>, clock: Arc<dyn Clock>) -> OkxResult<Self> {
        let mut builder = reqwest::Client::builder().timeout(config.timeout);
        if let Some(proxy) = &config.proxy {
            builder = builder.proxy(
                reqwest::Proxy::all(proxy)
                    .map_err(|error| OkxError::InvalidConfiguration(error.to_string()))?,
            );
        }
        Ok(Self {
            client: builder.build()?,
            config,
            clock,
        })
    }

    /// Executes exact canonical bytes and returns raw response bytes for typed decoding.
    pub(crate) async fn execute(&self, request: CanonicalRequest) -> OkxResult<HttpResponse> {
        let url = format!(
            "{}{}",
            self.config.environment.rest_base(),
            request.path_and_query
        );
        let mut builder = self
            .client
            .request(request.method.clone(), url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(request.body.clone());
        let simulated = match self.config.environment {
            Environment::Demo => "1",
            Environment::Live(_) => "0",
            Environment::Custom {
                simulated: true, ..
            } => "1",
            Environment::Custom {
                simulated: false, ..
            } => "0",
        };
        builder = builder.header("x-simulated-trading", simulated);
        if request.requires_auth {
            let credentials = self
                .config
                .credentials
                .as_ref()
                .ok_or(OkxError::MissingCredentials)?;
            let (signature, timestamp) = sign_rest(credentials, &request, self.clock.now())?;
            for (name, value) in private_headers(credentials, signature, timestamp) {
                builder = builder.header(name, value);
            }
        }
        let response = builder.send().await?;
        if response.status().as_u16() == 429 {
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .map(std::time::Duration::from_secs);
            return Err(OkxError::RateLimited { retry_after });
        }
        if !response.status().is_success() {
            let message = response
                .status()
                .canonical_reason()
                .map(str::to_owned)
                .unwrap_or_else(|| "HTTP failure".to_owned());
            return Err(OkxError::Exchange {
                code: response.status().as_u16().to_string(),
                message,
                request_id: response
                    .headers()
                    .get("x-request-id")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned),
            });
        }
        let metadata = ResponseMetadata::from_headers(response.headers());
        Ok(HttpResponse {
            body: response.bytes().await?.to_vec(),
            metadata,
        })
    }
}
