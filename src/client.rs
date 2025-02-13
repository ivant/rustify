//! Contains the [Client] trait for executing
//! [Endpoints][crate::endpoint::Endpoint].
use crate::errors::ClientError;
use async_trait::async_trait;
use http::{Request, Response};
use std::ops::RangeInclusive;

/// An array of HTTP response codes which indicate a successful response
pub const HTTP_SUCCESS_CODES: RangeInclusive<u16> = 200..=208;

/// Represents an HTTP client which is capable of executing
/// [Endpoints][crate::endpoint::Endpoint] by sending the [Request] generated
/// by the Endpoint and returning a [Response].
#[async_trait]
pub trait Client: Sync + Send {
    /// Sends the given [Request] and returns a [Response]. Implementations
    /// should consolidate all errors into the [ClientError] type.
    async fn send(&self, req: Request<Vec<u8>>) -> Result<Response<Vec<u8>>, ClientError>;

    /// Returns the base URL the client is configured with. This is used for
    /// creating the fully qualified URLs used when executing
    /// [Endpoints][crate::endpoint::Endpoint].
    fn base(&self) -> &str;

    /// This method provides a common interface to
    /// [Endpoints][crate::endpoint::Endpoint] for execution.
    // TODO: remove the allow when the upstream clippy issue is fixed:
    // <https://github.com/rust-lang/rust-clippy/issues/12281>
    #[allow(clippy::blocks_in_conditions)]
    #[instrument(skip(self, req), fields(uri=%req.uri(), method=%req.method()), err)]
    async fn execute(&self, req: Request<Vec<u8>>) -> Result<Response<Vec<u8>>, ClientError> {
        debug!(
            name: "sending_request",
            body_len=req.body().len(),
            "Sending Request",
        );
        let response = self.send(req).await?;
        let status = response.status();
        debug!(
            name: "response_received",
            status=status.as_u16(),
            response_len=response.body().len(),
            is_error=status.is_client_error() || status.is_server_error(),
            "Response Received",
        );

        // Check response
        if !HTTP_SUCCESS_CODES.contains(&response.status().as_u16()) {
            return Err(ClientError::ServerResponseError {
                code: response.status().as_u16(),
                content: String::from_utf8(response.body().to_vec()).ok(),
            });
        }

        // Parse response content
        Ok(response)
    }
}

/// Client that wraps another client and adds a bearer token authentication header to each request.
pub struct BearerTokenAuthClient<C: Client> {
    token: String,
    client: C,
}

impl <C: Client> BearerTokenAuthClient<C> {
    /// Construct from an arbitrary client and a bearer token.
    pub fn new(client: C, token: &str) -> Self {
        Self { client, token: token.to_string() }
    }
}

#[async_trait::async_trait]
impl <C: Client> Client for BearerTokenAuthClient<C> {
    fn base(&self) ->  &str {
        self.client.base()
    }

    async fn send(&self, mut req: Request<Vec<u8>>) ->  Result<Response<Vec<u8>>, ClientError> {
        let bearer = format!("Bearer {}", self.token);
        match http::HeaderValue::from_str(&bearer) {
            Ok(mut bearer) => {
                bearer.set_sensitive(true);
                req.headers_mut().insert("Authorization", bearer);
                self.client.send(req).await
            }
            Err(e) => Err(ClientError::GenericError { source: e.into() })
        }
    }
}
