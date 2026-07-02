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
    k("SenderSubID", "identity", Rec),
    k("SenderLocationID", "identity", Rec),
    k("TargetCompID", "identity", Impl),
    k("TargetSubID", "identity", Rec),
    k("TargetLocationID", "identity", Rec),
    k("SessionQualifier", "identity", Rec),
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
    k(
        "ContinueInitializationOnError",
        "session",
        Rec,
    ),
    k("LogMessageWhenSessionNotFound", "acceptor", Impl),
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
    k("SocketAcceptProtocol", "acceptor", Rec),
    k("AcceptorTemplate", "acceptor", Impl),
    k("DynamicSession", "acceptor", Impl),
    k("AllowedRemoteAddresses", "acceptor", Impl),
    // Initiator
    k("SocketConnectHost", "initiator", Impl),
    k("SocketConnectPort", "initiator", Impl),
    // Numbered backup endpoints (SocketConnectHost1/Port1, SocketConnectHost2/Port2, ...) for
    // multi-endpoint failover (FR-019); N=1 shown as the representative entry.
    k("SocketConnectHost1", "initiator", Impl),
    k("SocketConnectPort1", "initiator", Impl),
    k("SocketConnectProtocol", "initiator", Rec),
    k("SocketConnectTimeout", "initiator", Rec),
    k("SocketLocalHost", "initiator", Rec),
    k("SocketLocalPort", "initiator", Rec),
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
    // SQL (JDBC-equivalent) store/log — available behind the `sql` feature
    k("JdbcDriver", "sql", Rec),
    k("JdbcURL", "sql", Rec),
    k("JdbcUser", "sql", Rec),
    k("JdbcPassword", "sql", Rec),
    k("JdbcDataSourceName", "sql", Rec),
    k("JdbcStoreMessagesTableName", "sql", Rec),
    k("JdbcStoreSessionsTableName", "sql", Rec),
    k("JdbcLogIncomingTable", "sql", Rec),
    k("JdbcLogOutgoingTable", "sql", Rec),
    k("JdbcLogEventTable", "sql", Rec),
    k("JdbcLogHeartBeats", "sql", Rec),
    k("JdbcMaxActiveConnection", "sql", Rec),
    k("JdbcMaxConnectionLifeTime", "sql", Rec),
    k("JdbcMinIdleConnection", "sql", Rec),
    k("JdbcConnectionTimeout", "sql", Rec),
    k("JdbcConnectionIdleTimeout", "sql", Rec),
    k("JdbcConnectionKeepaliveTime", "sql", Rec),
    k("JdbcConnectionTestQuery", "sql", Rec),
    k("JdbcSessionIdDefaultPropertyValue", "sql", Rec),
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
