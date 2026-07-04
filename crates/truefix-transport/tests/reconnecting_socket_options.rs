//! T055/T056 (US2, feature 007): the plain (non-TLS, non-failover) reconnecting initiator
//! (`connect_initiator_reconnecting_multi`) applies the same socket options every other
//! connection path applies (BUG-39/FR-021) — previously the connected `stream` was handed straight
//! to `run_connection` without ever calling `services.socket_options.apply(&stream)`.
//!
//! **Testing-approach note**: a true black-box behavioral test (observe some socket-level effect
//! of an applied option from outside the process) was attempted and abandoned. `SO_LINGER(0)`
//! seemed promising (an abortive RST-on-close vs. a graceful FIN is externally observable) but
//! `run_connection`'s shutdown tail unconditionally calls a graceful `write_half.shutdown()`
//! before the stream is ever dropped on *every* exit path, which neutralizes `SO_LINGER`'s effect
//! regardless of whether socket options were applied. `TCP_NODELAY`'s effect (Nagle's algorithm
//! coalescing small writes) was also considered, but confirmed experimentally to not manifest
//! reliably over a loopback connection: Nagle's delay window is bounded by ACK round-trip time,
//! which on loopback is sub-millisecond regardless of the `nodelay` setting, so no meaningful,
//! non-flaky timing difference exists to observe. Every other option in `SocketOptions` is either
//! similarly unobservable from the peer's side (`keep_alive`, buffer sizes, `oob_inline`,
//! `traffic_class`) or acceptor-listener-only (`reuse_address`). Given this, the test below
//! verifies the fix directly against the source: `connect_initiator_reconnecting_multi`'s
//! connection-establishment branch must contain a call to `services.socket_options.apply`, mirroring
//! every sibling connection path in the same file (already confirmed by inspection to number six
//! before this fix). `SocketOptions::apply` itself is already behaviorally tested in
//! `socket_options.rs`'s `full_option_set_applies_without_error` (verifies `nodelay()` reflects a
//! *directly*-applied option on a stream the test owns outright) — what's missing, and what this
//! guards, is specifically that *this* code path invokes it at all.

#[test]
fn connect_initiator_reconnecting_multi_applies_socket_options() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"))
        .expect("read truefix-transport/src/lib.rs");

    let fn_start = source
        .find("pub fn connect_initiator_reconnecting_multi<A>(")
        .expect("connect_initiator_reconnecting_multi must exist in src/lib.rs");
    // The function body is a few hundred bytes; a generous fixed window comfortably covers it
    // without needing a full brace-matching parser.
    let window = &source[fn_start..(fn_start + 3000).min(source.len())];
    let fn_end = window
        .find("\npub fn connect_initiator_reconnecting_multi_tls")
        .unwrap_or(window.len());
    let body = &window[..fn_end];

    assert!(
        body.contains("socket_options.apply"),
        "connect_initiator_reconnecting_multi must call services.socket_options.apply(&stream) \
         on each successful connect, matching every other connection path in this file"
    );
}
