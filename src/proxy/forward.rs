use axum::body::Body;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::Method;
use axum::http::Uri;
use axum::response::Response;
use bytes::Bytes;
use futures::TryStreamExt;

use crate::error::ProxyError;
use crate::proxy::state::{Alexandrie, ResolvedAuth};

/// Forwards incoming messages to the upstream API.
///
/// This is the main proxy handler. It:
/// 1. Reads the request body
/// 2. Clones auth + upstream URL from shared state (read lock, then drop)
/// 3. Builds a reqwest request with injected authentication
/// 4. Sends to upstream
/// 5. Checks content-type for "text/event-stream" to decide SSE bridge vs JSON
pub async fn forward_messages(
    State(alexandrie): State<Alexandrie>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    // Grab what we need from state, then release the lock immediately.
    let (auth, upstream_url, client, effective_model) = {
        let mut state = alexandrie.write().await;
        state.stats.requests_forwarded += 1;
        let auth = state.active_auth.clone();
        let upstream_url = state.upstream_url.clone();
        let client = state.client.clone();
        let effective_model = effective_model_for(&state);
        (auth, upstream_url, client, effective_model)
    };

    // Build the upstream URL.
    let url = format!("{}/v1/messages", upstream_url.trim_end_matches('/'));

    let body = apply_model_override(body, effective_model.as_deref());

    // Build the outgoing request.
    let mut req_builder = client
        .post(&url)
        .header("content-type", "application/json")
        .body(body);

    // Inject auth headers.
    req_builder = inject_auth(req_builder, &auth);

    // Forward all client headers except ones we manage ourselves.
    for (name, value) in &headers {
        let n = name.as_str().to_ascii_lowercase();
        if n == "host" || n == "x-api-key" || n == "authorization" || n == "content-length" {
            continue;
        }
        req_builder = req_builder.header(name, value);
    }

    // Send upstream.
    let response = req_builder.send().await?;

    // Check if the response is SSE.
    let is_sse = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.contains("text/event-stream"))
        .unwrap_or(false);

    let status = response.status();
    let resp_headers = response.headers().clone();

    if is_sse {
        // SSE bridge: stream bytes through zero-copy.
        let stream = response
            .bytes_stream()
            .map_err(|e| std::io::Error::other(e));
        let body = Body::from_stream(stream);

        let mut builder = Response::builder().status(status.as_u16());
        for (key, value) in resp_headers.iter() {
            builder = builder.header(key, value);
        }
        builder.body(body).map_err(|e| ProxyError::Internal(e.to_string()))
    } else {
        // JSON response: read full body and return.
        let resp_bytes = response.bytes().await?;
        let mut builder = Response::builder().status(status.as_u16());
        for (key, value) in resp_headers.iter() {
            builder = builder.header(key, value);
        }
        builder
            .body(Body::from(resp_bytes))
            .map_err(|e| ProxyError::Internal(e.to_string()))
    }
}

/// Generic JSON forwarding for arbitrary paths.
pub async fn forward_json(
    State(alexandrie): State<Alexandrie>,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    let (auth, upstream_url, client, effective_model) = {
        let mut state = alexandrie.write().await;
        state.stats.requests_forwarded += 1;
        let effective_model = effective_model_for(&state);
        (
            state.active_auth.clone(),
            state.upstream_url.clone(),
            state.client.clone(),
            effective_model,
        )
    };

    let path = uri.path();
    let url = format!("{}{}", upstream_url.trim_end_matches('/'), path);
    let body = apply_model_override(body, effective_model.as_deref());

    let mut req_builder = client
        .post(&url)
        .header("content-type", "application/json")
        .body(body);

    req_builder = inject_auth(req_builder, &auth);

    // Forward all client headers except ones we manage ourselves.
    for (name, value) in &headers {
        let n = name.as_str().to_ascii_lowercase();
        if n == "host" || n == "x-api-key" || n == "authorization" || n == "content-length" {
            continue;
        }
        req_builder = req_builder.header(name, value);
    }

    let response = req_builder.send().await?;
    let status = response.status();
    let resp_headers = response.headers().clone();
    let resp_bytes = response.bytes().await?;

    let mut builder = Response::builder().status(status.as_u16());
    for (key, value) in resp_headers.iter() {
        builder = builder.header(key, value);
    }
    builder
        .body(Body::from(resp_bytes))
        .map_err(|e| ProxyError::Internal(e.to_string()))
}

/// Handles GET requests (like /v1/models) and forwards them upstream.
pub async fn forward_get(
    State(alexandrie): State<Alexandrie>,
    headers: HeaderMap,
    uri: Uri,
) -> Result<Response, ProxyError> {
    let (auth, upstream_url, client) = {
        let mut state = alexandrie.write().await;
        state.stats.requests_forwarded += 1;
        (state.active_auth.clone(), state.upstream_url.clone(), state.client.clone())
    };

    let path = uri.path();
    let url = format!("{}{}", upstream_url.trim_end_matches('/'), path);

    let mut req_builder = client.get(&url);
    req_builder = inject_auth(req_builder, &auth);

    // Forward all client headers except ones we manage ourselves.
    for (name, value) in &headers {
        let n = name.as_str().to_ascii_lowercase();
        if n == "host" || n == "x-api-key" || n == "authorization" || n == "content-length" {
            continue;
        }
        req_builder = req_builder.header(name, value);
    }

    let response = req_builder.send().await?;
    let status = response.status();
    let resp_headers = response.headers().clone();
    let resp_bytes = response.bytes().await?;

    let mut builder = Response::builder().status(status.as_u16());
    for (key, value) in resp_headers.iter() {
        builder = builder.header(key, value);
    }
    builder.body(Body::from(resp_bytes)).map_err(|e| ProxyError::Internal(e.to_string()))
}

/// Catches any unmatched request and forwards it upstream.
pub async fn forward_fallback(
    State(alexandrie): State<Alexandrie>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    let (auth, upstream_url, client, effective_model) = {
        let mut state = alexandrie.write().await;
        state.stats.requests_forwarded += 1;
        let effective_model = effective_model_for(&state);
        (state.active_auth.clone(), state.upstream_url.clone(), state.client.clone(), effective_model)
    };

    let path = uri.path();
    let url = format!("{}{}", upstream_url.trim_end_matches('/'), path);
    let body = apply_model_override(body, effective_model.as_deref());

    let mut req_builder = client.request(method, &url)
        .header("content-type", "application/json")
        .body(body);

    req_builder = inject_auth(req_builder, &auth);

    // Forward all client headers except ones we manage ourselves.
    for (name, value) in &headers {
        let n = name.as_str().to_ascii_lowercase();
        if n == "host" || n == "x-api-key" || n == "authorization" || n == "content-length" {
            continue;
        }
        req_builder = req_builder.header(name, value);
    }

    let response = req_builder.send().await?;
    let is_sse = response.headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.contains("text/event-stream"))
        .unwrap_or(false);

    let status = response.status();
    let resp_headers = response.headers().clone();

    if is_sse {
        let stream = response.bytes_stream().map_err(|e| std::io::Error::other(e));
        let body = Body::from_stream(stream);
        let mut builder = Response::builder().status(status.as_u16());
        for (key, value) in resp_headers.iter() {
            builder = builder.header(key, value);
        }
        builder.body(body).map_err(|e| ProxyError::Internal(e.to_string()))
    } else {
        let resp_bytes = response.bytes().await?;
        let mut builder = Response::builder().status(status.as_u16());
        for (key, value) in resp_headers.iter() {
            builder = builder.header(key, value);
        }
        builder.body(Body::from(resp_bytes)).map_err(|e| ProxyError::Internal(e.to_string()))
    }
}

/// Resolves the effective model for the current request: an explicit runtime
/// override wins; otherwise falls back to the active profile's configured
/// model, matching what `control::get_model` reports.
fn effective_model_for(state: &crate::proxy::state::ProxyState) -> Option<String> {
    state.model_override.clone().or_else(|| {
        state
            .config
            .profiles
            .get(&state.active_profile)
            .and_then(|p| p.model())
            .map(str::to_string)
    })
}

/// Rewrites the `model` field of a JSON request body, if a model is set and
/// the body actually decodes to a JSON object. Non-object bodies (arrays,
/// scalars, or invalid JSON) are passed through unchanged rather than
/// panicking on `Value`'s `IndexMut`, which requires an object.
fn apply_model_override(body: Bytes, model: Option<&str>) -> Bytes {
    let Some(model) = model else { return body };
    match serde_json::from_slice::<serde_json::Value>(&body) {
        Ok(mut json) if json.is_object() => {
            json["model"] = serde_json::Value::String(model.to_string());
            Bytes::from(serde_json::to_vec(&json).unwrap_or_else(|_| body.to_vec()))
        }
        _ => body,
    }
}

/// Injects the appropriate authentication header into a request builder.
fn inject_auth(
    builder: reqwest::RequestBuilder,
    auth: &ResolvedAuth,
) -> reqwest::RequestBuilder {
    match auth {
        ResolvedAuth::BearerToken(token) => {
            builder.header("authorization", format!("Bearer {}", token))
        }
        ResolvedAuth::Passthrough { token } => {
            if let Some(t) = token {
                builder.header("authorization", format!("Bearer {}", t))
            } else {
                builder
            }
        }
    }
}
