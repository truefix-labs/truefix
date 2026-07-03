//! The Appendix A configuration-key registry (FR-I2, SC-004).
//!
//! Every QuickFIX/J configuration key in the spec's Appendix A baseline appears here with a
//! [`Stance`]: `Implemented` (recognized and honored), `Recognized` (parsed into settings;
//! behavior partial/pending in the current stage), or `Unsupported` (intentionally not supported,
//! with a reason). This guarantees no key is silently unrecognized.

/// How TrueFix treats a configuration key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stance {
    /// Recognized and honored by the engine.
    Implemented,
    /// Recognized and parsed into settings; behavior is partial or pending.
    Recognized,
    /// Recognized but intentionally not supported in v1, with a reason.
    Unsupported(&'static str),
}

/// Metadata about one configuration key.
#[derive(Debug, Clone, Copy)]
pub struct KeyInfo {
    /// The key name as it appears in a `.cfg`.
    pub name: &'static str,
    /// The Appendix A group it belongs to.
    pub group: &'static str,
    /// How TrueFix treats it.
    pub stance: Stance,
}

const fn k(name: &'static str, group: &'static str, stance: Stance) -> KeyInfo {
    KeyInfo {
        name,
        group,
        stance,
    }
}

use Stance::{Implemented as Impl, Recognized as Rec, Unsupported as Unsup};

/// The full Appendix A configuration-key baseline.
pub const APPENDIX_A_KEYS: &[KeyInfo] = &[
    // Session identity & type
    k("BeginString", "identity", Impl),
    k("SenderCompID", "identity", Impl),
    k("SenderSubID", "identity", Impl),
    k("SenderLocationID", "identity", Impl),
    k("TargetCompID", "identity", Impl),
    k("TargetSubID", "identity", Impl),
    k("TargetLocationID", "identity", Impl),
    k("SessionQualifier", "identity", Impl),
    k("ConnectionType", "identity", Impl),
    k("Description", "identity", Rec),
    k("DefaultApplVerID", "identity", Impl),
    // Dictionary & validation
    k("UseDataDictionary", "validation", Impl),
    k("DataDictionary", "validation", Impl),
    k("AppDataDictionary", "validation", Impl),
    k("TransportDataDictionary", "validation", Impl),
    k("ValidateFieldsOutOfOrder", "validation", Impl),
    k("ValidateFieldsHaveValues", "validation", Impl),
    k("ValidateUnorderedGroupFields", "validation", Impl),
    k("ValidateUserDefinedFields", "validation", Impl),
    k("ValidateIncomingMessage", "validation", Impl),
    k("ValidateSequenceNumbers", "validation", Impl),
    k("ValidateChecksum", "validation", Impl),
    k("AllowUnknownMsgFields", "validation", Impl),
    k("CheckLatency", "validation", Impl),
    k("MaxLatency", "validation", Impl),
    k("CheckCompID", "validation", Impl),
    k("RejectGarbledMessage", "validation", Impl),
    k("RejectInvalidMessage", "validation", Impl),
    k(
        "RejectMessageOnUnhandledException",
        "validation",
        Unsup("TrueFix's typed-error architecture (Constitution Principle I: no panics on critical paths) has no unhandled-exception class this would apply to"),
    ),
    k("FirstFieldInGroupIsDelimiter", "validation", Impl),
    // Session behavior
    k("HeartBtInt", "session", Impl),
    k("HeartBeatTimeoutMultiplier", "session", Impl),
    k("DisableHeartBeatCheck", "session", Impl),
    k("TestRequestDelayMultiplier", "session", Impl),
    k("LogonTimeout", "session", Impl),
    k("LogoutTimeout", "session", Impl),
    k("LogonTag", "session", Impl),
    k("ResetOnLogon", "session", Impl),
    k("ResetOnLogout", "session", Impl),
    k("ResetOnDisconnect", "session", Impl),
    k("ResetOnError", "session", Impl),
    k("RefreshOnLogon", "session", Impl),
    k("PersistMessages", "session", Impl),
    k("ResendRequestChunkSize", "session", Impl),
    k("SendRedundantResendRequests", "session", Impl),
    k(
        "ClosedResendInterval",
        "session",
        Unsup("TrueFix's session state machine processes one event at a time with no concurrent resend-servicing path, so there is no open/closed-interval race for this to resolve"),
    ),
    k("EnableLastMsgSeqNumProcessed", "session", Impl),
    k("EnableNextExpectedMsgSeqNum", "session", Impl),
    k("RequiresOrigSendingTime", "session", Impl),
    k("AllowPosDup", "session", Impl),
    k("ForceResendWhenCorruptedStore", "session", Impl),
    k("DisconnectOnError", "session", Impl),
    k("TimeStampPrecision", "session", Impl),
    k(
        "MaxScheduledWriteRequests",
        "session",
        Unsup("the session state machine returns actions synchronously for the transport to write immediately; there is no internal outbound write queue for this to bound"),
    ),
    // US4, feature 004 (FR-005): `.cfg` → `SessionConfig.continue_initialization_on_error`, and
    // `Engine::start`'s multi-session bring-up (both config-resolution and runtime startup
    // failures) actually honors it — was `Recognized` (round-tripped but inert) through feature
    // 003.
    k("ContinueInitializationOnError", "session", Impl),
    k("LogMessageWhenSessionNotFound", "acceptor", Impl),
    // Backpressure (US14, FR-019): bounds the application-message inbound channel; admin/session
    // messages always travel on a separate, unbounded channel so they're never starved by a full
    // application channel. Absent (the default) preserves pre-US14 single-channel behavior exactly.
    k("InChanCapacity", "session", Impl),
    // Scheduling
    k("StartTime", "scheduling", Impl),
    k("EndTime", "scheduling", Impl),
    k("StartDay", "scheduling", Impl),
    k("EndDay", "scheduling", Impl),
    k("Weekdays", "scheduling", Impl),
    k("TimeZone", "scheduling", Impl),
    k("NonStopSession", "scheduling", Impl),
    // Acceptor / dynamic session
    k("SocketAcceptAddress", "acceptor", Impl),
    k("SocketAcceptPort", "acceptor", Impl),
    k(
        "SocketAcceptProtocol",
        "acceptor",
        Unsup("selects between QFJ's SOCKET and VM_PIPE (in-JVM-process) transport factories; VM_PIPE has no meaningful Rust equivalent (US8, feature 005)"),
    ),
    // `AcceptorTemplate`/`DynamicSession`/`AllowedRemoteAddresses` were already marked `Impl`
    // before feature 005, but `builder.rs` never read any of the three and `Engine::start` only
    // ever bound one single-session `Acceptor` per `[SESSION]` block — so a `.cfg` setting them had
    // no effect at all (BUG-03). Feature 005 (US2, FR-006/006a) makes the marking accurate:
    // `Engine::start` now groups `[SESSION]` blocks sharing a bind address into one
    // `AcceptorBuilder`, registering each group's dynamic template and the union of its members'
    // allow-list entries.
    k("AcceptorTemplate", "acceptor", Impl),
    k("DynamicSession", "acceptor", Impl),
    k("AllowedRemoteAddresses", "acceptor", Impl),
    // Initiator
    k("SocketConnectHost", "initiator", Impl),
    k("SocketConnectPort", "initiator", Impl),
    // Numbered backup endpoints (SocketConnectHost1/Port1, SocketConnectHost2/Port2, ...) for
    // multi-endpoint failover (FR-019); N=1 shown as the representative entry. Already marked
    // `Impl` since feature 002 (parsed into `ResolvedSession.failover_addresses`), but until
    // feature 004 (US1, GAP-02) `Engine::start` never actually reconnected through them — only a
    // one-shot `connect_initiator` was used. US1 makes the marking accurate: `Engine::start` now
    // routes to `connect_initiator_reconnecting_multi[_tls]` whenever `failover_addresses` is
    // non-empty (proxy+failover is the one unsupported combination — logged and falls back to the
    // existing proxy path).
    k("SocketConnectHost1", "initiator", Impl),
    k("SocketConnectPort1", "initiator", Impl),
    k(
        "SocketConnectProtocol",
        "initiator",
        Unsup("selects between QFJ's SOCKET and VM_PIPE (in-JVM-process) transport factories; VM_PIPE has no meaningful Rust equivalent (US8, feature 005)"),
    ),
    k("SocketConnectTimeout", "initiator", Impl),
    k("SocketLocalHost", "initiator", Impl),
    k("SocketLocalPort", "initiator", Impl),
    k("ReconnectInterval", "initiator", Impl),
    // Socket options (FR-019)
    k("SocketKeepAlive", "socket", Impl),
    k("SocketTcpNoDelay", "socket", Impl),
    k("SocketReuseAddress", "socket", Impl),
    k("SocketLinger", "socket", Impl),
    k("SocketOobInline", "socket", Impl),
    k("SocketReceiveBufferSize", "socket", Impl),
    k("SocketSendBufferSize", "socket", Impl),
    k("SocketTrafficClass", "socket", Impl),
    k("SocketSynchronousWrites", "socket", Impl),
    k("SocketSynchronousWriteTimeout", "socket", Impl),
    // SSL/TLS (FR-017: config-driven rustls, PEM-based — not Java keystores)
    k("SocketUseSSL", "ssl", Impl),
    k("EnabledProtocols", "ssl", Impl),
    k("CipherSuites", "ssl", Impl),
    k(
        "KeyStoreType",
        "ssl",
        Unsup("TrueFix uses PEM files via rustls, not Java keystores (JKS/PKCS12)"),
    ),
    k(
        "KeyManagerFactoryAlgorithm",
        "ssl",
        Unsup("Java KeyManagerFactory concept; not applicable to rustls"),
    ),
    k(
        "TrustManagerFactoryAlgorithm",
        "ssl",
        Unsup("Java TrustManagerFactory concept; not applicable to rustls"),
    ),
    k(
        "TrustStoreType",
        "ssl",
        Unsup("TrueFix uses PEM CA files via rustls, not Java keystores (JKS/PKCS12)"),
    ),
    k("SocketKeyStore", "ssl", Impl),
    k("SocketKeyStoreBytes", "ssl", Impl),
    k(
        "SocketKeyStorePassword",
        "ssl",
        Unsup("TrueFix's PEM-based key store is not password-encrypted"),
    ),
    k("SocketTrustStore", "ssl", Impl),
    k("SocketTrustStoreBytes", "ssl", Impl),
    k(
        "SocketTrustStorePassword",
        "ssl",
        Unsup("TrueFix's PEM-based trust store is not password-encrypted"),
    ),
    k("NeedClientAuth", "ssl", Impl),
    k("EndpointIdentificationAlgorithm", "ssl", Rec),
    k("UseSNI", "ssl", Impl),
    k("SNIHostName", "ssl", Impl),
    // Proxy (US12; FR-015/FR-016)
    k("UseTCPProxy", "proxy", Impl),
    k("TrustedProxyAddresses", "proxy", Impl),
    k("ProxyType", "proxy", Impl),
    k(
        "ProxyVersion",
        "proxy",
        Unsup("superseded by ProxyType (Socks4/Socks5/HttpConnect covers this project's proxy-type scope)"),
    ),
    k("ProxyHost", "proxy", Impl),
    k("ProxyPort", "proxy", Impl),
    k("ProxyUser", "proxy", Impl),
    k("ProxyPassword", "proxy", Impl),
    k(
        "ProxyDomain",
        "proxy",
        Unsup("Windows NTLM-proxy-specific field; out of this project's proxy-type scope (SOCKS4/SOCKS5/HTTP CONNECT)"),
    ),
    k(
        "ProxyWorkstation",
        "proxy",
        Unsup("Windows NTLM-proxy-specific field; out of this project's proxy-type scope (SOCKS4/SOCKS5/HTTP CONNECT)"),
    ),
    // File store (FR-025)
    k("FileStorePath", "file-store", Impl),
    k("FileStoreSync", "file-store", Impl),
    k("FileStoreMaxCachedMsgs", "file-store", Impl),
    // File log (FR-026)
    k("FileLogPath", "file-log", Impl),
    k("FileLogHeartbeats", "file-log", Impl),
    k("FileIncludeMilliseconds", "file-log", Impl),
    k("FileIncludeTimeStampForMessages", "file-log", Impl),
    // Screen log
    k("ScreenLogShowEvents", "screen-log", Rec),
    k("ScreenLogShowHeartBeats", "screen-log", Rec),
    k("ScreenLogShowIncoming", "screen-log", Rec),
    k("ScreenLogShowOutgoing", "screen-log", Rec),
    k("ScreenIncludeMilliseconds", "screen-log", Rec),
    // Facade (SLF4J-equivalent) log
    k(
        "SLF4JLogEventCategory",
        "facade-log",
        Unsup("TrueFix logs via the `tracing` facade; SLF4J category keys are not applicable"),
    ),
    k(
        "SLF4JLogErrorEventCategory",
        "facade-log",
        Unsup("TrueFix logs via the `tracing` facade; SLF4J category keys are not applicable"),
    ),
    k(
        "SLF4JLogIncomingMessageCategory",
        "facade-log",
        Unsup("TrueFix logs via the `tracing` facade; SLF4J category keys are not applicable"),
    ),
    k(
        "SLF4JLogOutgoingMessageCategory",
        "facade-log",
        Unsup("TrueFix logs via the `tracing` facade; SLF4J category keys are not applicable"),
    ),
    k("SLF4JLogPrependSessionID", "facade-log", Rec),
    k("SLF4JLogHeartbeats", "facade-log", Rec),
    // SQL (JDBC-equivalent) store/log — PostgreSQL/MySQL/SQLite behind the `sql` feature
    // (`SqlStore`/`SqlLog`, via `sqlx`) and MSSQL behind the separate `mssql` feature
    // (`MssqlStore`/`MssqlLog`, via `tiberius` — sqlx has no official MSSQL driver).
    //
    // `JdbcURL` itself is `Implemented` (US3, feature 004, FR-003; extended US2, feature 005,
    // BUG-04/FR-003): `builder.rs`'s `resolve_store`/`resolve_log` dispatch on the URL's scheme
    // prefix — both TrueFix's own sqlx-native form (`postgres://`/`mysql://`/`sqlite:` vs.
    // `mssql://`/`sqlserver://`) and, since feature 005, the real JDBC form QuickFIX/J's own `.cfg`
    // files actually use (`jdbc:postgresql://`/`jdbc:mysql://`/`jdbc:sqlite:`/`jdbc:h2:` vs.
    // `jdbc:sqlserver://`) — a scheme-sniffing equivalent to QuickFIX/J's `JdbcDriver`-class-based
    // registry dispatch, not a literal port of it (this codebase has no driver-class registry to
    // mirror). The four `JdbcLog*` keys consumed by that same dispatch (table names + heartbeat
    // filter, feeding the new `SqlLogSpec`) are `Implemented` too. `JdbcUser`/`JdbcPassword` are
    // `Implemented` since feature 005 (BUG-04/FR-004): spliced into a credential-less `JdbcURL`'s
    // authority before it reaches `StoreConfig::Sql`/`Mssql`/`SqlLogSpec`, matching how real
    // QuickFIX/J `.cfg` files supply them (`JdbcUtil.java:69-72`) — an already-credentialed URL
    // (TrueFix's own `postgres://user:pass@host/db` form) is never double-spliced. `JdbcDriver`
    // stays `Recognized`: the URL's own scheme already carries this information, so there's
    // nothing left to configure once the URL is present. `JdbcDataSourceName` (feature 005, US8)
    // moves to `Unsupported` — see that key's own entry below, not here, since it's part of a
    // different (JNDI) mechanism than `JdbcDriver`/`JdbcUser`/`JdbcPassword`.
    // `JdbcStoreMessagesTableName`/`JdbcStoreSessionsTableName`/
    // `JdbcSessionIdDefaultPropertyValue` and the `Jdbc*Connection*`/`JdbcMaxActiveConnection`/
    // `JdbcMinIdleConnection` pool-tuning keys moved `Recognized` → `Implemented` in US8 (feature
    // 005, FR-021): `StoreConfig::Sql`/`Mssql` gained optional `sessions_table`/`messages_table`/
    // `session_id`/`pool` fields (`None` preserves each backend's existing default), parsed by
    // `jdbc_table_name_keys`/`jdbc_pool_options` in `builder.rs`. Oracle (per spec 003's
    // Clarifications) is deferred rather than implemented — see `docs/parity-matrix.md`'s
    // "Feature 003 — US14" section for the license rationale.
    k("JdbcDriver", "sql", Rec),
    k("JdbcURL", "sql", Impl),
    k("JdbcUser", "sql", Impl),
    k("JdbcPassword", "sql", Impl),
    k(
        "JdbcDataSourceName",
        "sql",
        Unsup("part of QFJ's JNDI DataSource lookup mechanism, same as JndiContextFactory/JndiProviderURL below — JNDI has no Rust equivalent (US8, feature 005)"),
    ),
    k("JdbcStoreMessagesTableName", "sql", Impl),
    k("JdbcStoreSessionsTableName", "sql", Impl),
    k("JdbcLogIncomingTable", "sql", Impl),
    k("JdbcLogOutgoingTable", "sql", Impl),
    k("JdbcLogEventTable", "sql", Impl),
    k("JdbcLogHeartBeats", "sql", Impl),
    k("JdbcMaxActiveConnection", "sql", Impl),
    k("JdbcMaxConnectionLifeTime", "sql", Impl),
    k("JdbcMinIdleConnection", "sql", Impl),
    k("JdbcConnectionTimeout", "sql", Impl),
    k("JdbcConnectionIdleTimeout", "sql", Impl),
    // Parsed and stored (`SqlPoolOptions::keepalive`), but not wired into `sqlx`'s pool — `sqlx`
    // has no keepalive-probe concept distinct from idle_timeout/max_lifetime (both already
    // exposed as real keys above). Stays `Impl` since the value is genuinely reachable/stored,
    // matching this project's precedent for keys with no underlying mechanism to attach to.
    k("JdbcConnectionKeepaliveTime", "sql", Impl),
    k(
        "JdbcConnectionTestQuery",
        "sql",
        Unsup("sqlx's pool already validates connection liveliness automatically before handing one out (test_before_acquire, default true) — no string-based custom-query hook is exposed at the .cfg level for this to map onto (US8, feature 005)"),
    ),
    k("JdbcSessionIdDefaultPropertyValue", "sql", Impl),
    k(
        "JndiContextFactory",
        "sql",
        Unsup("JNDI data-source lookup is not applicable in Rust"),
    ),
    k(
        "JndiProviderURL",
        "sql",
        Unsup("JNDI data-source lookup is not applicable in Rust"),
    ),
    // Embedded-KV (Sleepycat/JE-equivalent) store — deferred for v1 (T093)
    k(
        "SleepycatDatabaseDir",
        "sleepycat",
        Unsup("Sleepycat/JE embedded-KV store deferred for v1; use the file or SQL store"),
    ),
    k(
        "SleepycatMessageDbName",
        "sleepycat",
        Unsup("Sleepycat/JE embedded-KV store deferred for v1; use the file or SQL store"),
    ),
    k(
        "SleepycatSequenceDbName",
        "sleepycat",
        Unsup("Sleepycat/JE embedded-KV store deferred for v1; use the file or SQL store"),
    ),
];

/// Look up the stance for a configuration key (case-sensitive, as in QuickFIX/J).
pub fn key_info(name: &str) -> Option<&'static KeyInfo> {
    APPENDIX_A_KEYS.iter().find(|k| k.name == name)
}
