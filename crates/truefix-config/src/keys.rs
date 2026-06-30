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
    k("ValidateFieldsOutOfOrder", "validation", Rec),
    k("ValidateFieldsHaveValues", "validation", Impl),
    k("ValidateUnorderedGroupFields", "validation", Rec),
    k("ValidateUserDefinedFields", "validation", Impl),
    k("ValidateIncomingMessage", "validation", Impl),
    k("ValidateSequenceNumbers", "validation", Impl),
    k("ValidateChecksum", "validation", Impl),
    k("AllowUnknownMsgFields", "validation", Impl),
    k("CheckLatency", "validation", Impl),
    k("MaxLatency", "validation", Impl),
    k("CheckCompID", "validation", Rec),
    k("RejectGarbledMessage", "validation", Rec),
    k("RejectInvalidMessage", "validation", Impl),
    k("RejectMessageOnUnhandledException", "validation", Rec),
    k("FirstFieldInGroupIsDelimiter", "validation", Rec),
    // Session behavior
    k("HeartBtInt", "session", Impl),
    k("HeartBeatTimeoutMultiplier", "session", Rec),
    k("DisableHeartBeatCheck", "session", Rec),
    k("TestRequestDelayMultiplier", "session", Rec),
    k("LogonTimeout", "session", Impl),
    k("LogoutTimeout", "session", Impl),
    k("LogonTag", "session", Rec),
    k("ResetOnLogon", "session", Impl),
    k("ResetOnLogout", "session", Impl),
    k("ResetOnDisconnect", "session", Impl),
    k("ResetOnError", "session", Rec),
    k("RefreshOnLogon", "session", Rec),
    k("PersistMessages", "session", Rec),
    k("ResendRequestChunkSize", "session", Impl),
    k("SendRedundantResendRequests", "session", Rec),
    k("ClosedResendInterval", "session", Rec),
    k("EnableLastMsgSeqNumProcessed", "session", Rec),
    k("EnableNextExpectedMsgSeqNum", "session", Impl),
    k("RequiresOrigSendingTime", "session", Impl),
    k("AllowPosDup", "session", Impl),
    k("ForceResendWhenCorruptedStore", "session", Rec),
    k("DisconnectOnError", "session", Rec),
    k("TimeStampPrecision", "session", Rec),
    k("MaxScheduledWriteRequests", "session", Rec),
    k("ContinueInitializationOnError", "session", Rec),
    k("LogMessageWhenSessionNotFound", "session", Rec),
    // Scheduling
    k("StartTime", "scheduling", Impl),
    k("EndTime", "scheduling", Impl),
    k("StartDay", "scheduling", Rec),
    k("EndDay", "scheduling", Rec),
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
    k("SocketConnectProtocol", "initiator", Rec),
    k("SocketConnectTimeout", "initiator", Rec),
    k("SocketLocalHost", "initiator", Rec),
    k("SocketLocalPort", "initiator", Rec),
    k("ReconnectInterval", "initiator", Impl),
    // Socket options
    k("SocketKeepAlive", "socket", Rec),
    k("SocketTcpNoDelay", "socket", Impl),
    k("SocketReuseAddress", "socket", Rec),
    k("SocketLinger", "socket", Rec),
    k("SocketOobInline", "socket", Rec),
    k("SocketReceiveBufferSize", "socket", Rec),
    k("SocketSendBufferSize", "socket", Rec),
    k("SocketTrafficClass", "socket", Rec),
    k("SocketSynchronousWrites", "socket", Rec),
    k("SocketSynchronousWriteTimeout", "socket", Rec),
    // SSL/TLS
    k("SocketUseSSL", "ssl", Impl),
    k("EnabledProtocols", "ssl", Impl),
    k("CipherSuites", "ssl", Impl),
    k("KeyStoreType", "ssl", Rec),
    k("KeyManagerFactoryAlgorithm", "ssl", Rec),
    k("TrustManagerFactoryAlgorithm", "ssl", Rec),
    k("TrustStoreType", "ssl", Rec),
    k("SocketKeyStore", "ssl", Impl),
    k("SocketKeyStorePassword", "ssl", Impl),
    k("SocketTrustStore", "ssl", Impl),
    k("SocketTrustStorePassword", "ssl", Impl),
    k("NeedClientAuth", "ssl", Impl),
    k("EndpointIdentificationAlgorithm", "ssl", Rec),
    k("UseSNI", "ssl", Impl),
    k("SNIHostName", "ssl", Impl),
    // Proxy
    k(
        "ProxyType",
        "proxy",
        Unsup("HTTP/SOCKS proxy is not yet implemented"),
    ),
    k(
        "ProxyVersion",
        "proxy",
        Unsup("HTTP/SOCKS proxy is not yet implemented"),
    ),
    k(
        "ProxyHost",
        "proxy",
        Unsup("HTTP/SOCKS proxy is not yet implemented"),
    ),
    k(
        "ProxyPort",
        "proxy",
        Unsup("HTTP/SOCKS proxy is not yet implemented"),
    ),
    k(
        "ProxyUser",
        "proxy",
        Unsup("HTTP/SOCKS proxy is not yet implemented"),
    ),
    k(
        "ProxyPassword",
        "proxy",
        Unsup("HTTP/SOCKS proxy is not yet implemented"),
    ),
    k(
        "ProxyDomain",
        "proxy",
        Unsup("HTTP/SOCKS proxy is not yet implemented"),
    ),
    k(
        "ProxyWorkstation",
        "proxy",
        Unsup("HTTP/SOCKS proxy is not yet implemented"),
    ),
    // File store
    k("FileStorePath", "file-store", Impl),
    k("FileStoreSync", "file-store", Rec),
    k("FileStoreMaxCachedMsgs", "file-store", Rec),
    // File log
    k("FileLogPath", "file-log", Impl),
    k("FileLogHeartbeats", "file-log", Rec),
    k("FileIncludeMilliseconds", "file-log", Rec),
    k("FileIncludeTimeStampForMessages", "file-log", Rec),
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
