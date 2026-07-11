// Each integration-test binary imports this shared fixture module, but only uses one transport.
// Keep the helpers available to their respective tests without treating the unused half as
// production dead code when the workspace runs Clippy with `-D warnings`.
#[allow(dead_code)]
pub mod http;

#[allow(dead_code)]
pub mod websocket;
