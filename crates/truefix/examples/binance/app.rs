//! The shared `Application` implementation: Binance's Ed25519 Logon signing (Binance's
//! non-standard auth scheme, on top of standard FIX.4.4), per-session endpoint behavior, and wire
//! traffic logging/pretty-printing. One `BinanceApp` is shared across every session this process
//! runs (order-entry, drop-copy, market-data), looked up by session id string.

use std::collections::HashMap;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use ed25519_dalek::Signer;

use truefix::core::{BusinessReject, DoNotSend, Reject};
use truefix::{Application, Field, Message, SessionId};

use crate::config::BinanceSessionExt;
use crate::display;

pub struct BinanceApp {
    /// Keyed by `"{begin_string}:{sender}->{target}"`, matching `SessionId`'s own `Display` impl.
    sessions: HashMap<String, BinanceSessionExt>,
}

impl BinanceApp {
    pub fn new(sessions: HashMap<String, BinanceSessionExt>) -> Self {
        Self { sessions }
    }

    fn ext(&self, session: &SessionId) -> Option<&BinanceSessionExt> {
        self.sessions.get(&session.to_string())
    }
}

fn field_str(message: &Message, tag: u32) -> String {
    message
        .header
        .get(tag)
        .or_else(|| message.body.get(tag))
        .and_then(|f| f.as_str().ok())
        .unwrap_or_default()
        .to_owned()
}

/// Prints one line of wire traffic to the screen: `SENT`/`RECV`, MsgType, and the full encoded
/// message with SOH swapped for `|` so it's readable. Called for every admin and app message in
/// both directions, so the terminal shows the whole session's traffic automatically.
fn log_traffic(direction: &str, session: &SessionId, message: &Message) {
    let readable = String::from_utf8_lossy(&message.encode()).replace('\u{1}', "|");
    tracing::info!(%session, direction, msg_type = ?message.msg_type(), "{readable}");
}

#[async_trait::async_trait]
impl Application for BinanceApp {
    async fn on_logon(&self, session: &SessionId) {
        tracing::info!(%session, "logon accepted");
    }

    async fn on_logout(&self, session: &SessionId) {
        tracing::info!(%session, "session logged out");
    }

    async fn to_admin(&self, message: &mut Message, session: &SessionId) {
        if message.msg_type() == Some("A") {
            match self.ext(session) {
                Some(ext) => {
                    // Payload per Binance's docs: MsgType SOH SenderCompID SOH TargetCompID SOH
                    // MsgSeqNum SOH SendingTime, values only (no tag=), signed with the Ed25519
                    // API-key's private key.
                    let payload = format!(
                        "A\u{1}{}\u{1}{}\u{1}{}\u{1}{}",
                        field_str(message, 49),
                        field_str(message, 56),
                        field_str(message, 34),
                        field_str(message, 52),
                    );
                    let signature = ext.signing_key.sign(payload.as_bytes());
                    let raw_data = BASE64.encode(signature.to_bytes());

                    message.body.set(Field::string(553, &ext.api_key));
                    message.body.set(Field::int(95, raw_data.len() as i64));
                    message.body.set(Field::string(96, &raw_data));
                    message
                        .body
                        .set(Field::string(25035, ext.message_handling.tag_value()));
                    if let Some(response_mode) = ext.response_mode {
                        message
                            .body
                            .set(Field::string(25036, response_mode.tag_value()));
                    }
                    if let Some(recv_window) = ext.recv_window {
                        message.body.set(Field::int(25000, recv_window as i64));
                    }
                    if ext.endpoint.is_drop_copy() {
                        message.body.set(Field::string(9406, "Y")); // DropCopyFlag, required by the drop-copy server
                    }
                }
                None => {
                    tracing::error!(%session, "no Binance settings found for this session id; Logon will fail authentication");
                }
            }
        }
        log_traffic("SENT", session, message);
    }

    async fn from_admin(&self, message: &Message, session: &SessionId) -> Result<(), Reject> {
        log_traffic("RECV", session, message);
        display::pretty_print(&session.to_string(), message);
        if message.msg_type() == Some("5") {
            tracing::warn!(%session, text = %field_str(message, 58), "received Logout");
        }
        Ok(())
    }

    async fn to_app(&self, message: &mut Message, session: &SessionId) -> Result<(), DoNotSend> {
        // RecvWindow(25000) is a per-request validity window, not just a Logon-time setting (per
        // Binance's docs) -- stamp the session's configured default onto every outbound app
        // message, same as to_admin already does for the Logon.
        if let Some(recv_window) = self.ext(session).and_then(|ext| ext.recv_window) {
            message.body.set(Field::int(25000, recv_window as i64));
        }
        log_traffic("SENT", session, message);
        Ok(())
    }

    async fn from_app(&self, message: &Message, session: &SessionId) -> Result<(), BusinessReject> {
        log_traffic("RECV", session, message);
        display::pretty_print(&session.to_string(), message);
        Ok(())
    }
}
