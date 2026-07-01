# Phase 1 Data Model: QuickFIX/J Parity Completion

Entities introduced or extended by this feature. Types are conceptual (Rust shapes finalised in
implementation). Relationships and validation rules trace to the spec FRs.

## EngineConfig (new — `truefix-config`)

The fully-resolved configuration parsed from a settings file (FR-013/014/015).

- `sessions: Vec<ResolvedSessionConfig>` — one per `[SESSION]`, with `[DEFAULT]` applied.
- Derived from `Settings` (existing per-section key→value map). Construction is all-or-nothing.

**ResolvedSessionConfig** (per session):

- `session: SessionConfig` (existing) — CompIDs, heartbeat, seq toggles, timestamp precision (new),
  `PersistMessages`, `HeartBeatTimeoutMultiplier`, `TestRequestDelayMultiplier` (new, FR-010).
- `connection: ConnectionType { Acceptor | Initiator }` — routes construction (FR-014).
- `store_spec: StoreSpec { Memory | File{dir} | CachedFile{dir,max,sync} | Sql{url,tables} | Noop }`.
- `log_spec: LogSpec { Screen{switches} | File{dir,switches} | Tracing | Sql{url} | Composite }`.
- `schedule: Option<Schedule>` (extended with weekly days, see below).
- `transport: TransportSpec` — socket options, connect endpoints (ordered), accept port.
- `tls: Option<TlsSpec>` — key/cert/CA paths, min version, SNI, `need_client_auth`.

**Validation** (FR-015): every recognised key with an invalid value, any missing required key, and any
unusable referenced resource (file path, URL) yields `ConfigError { key, session, kind }`; no partial
engine is started.

## ConfigError (new — `truefix-config`)

Typed startup error (Principle I; FR-015).

- `key: String`, `session: Option<SessionId>`, `kind: ConfigErrorKind`.
- `kind ∈ { MissingRequired, InvalidValue{reason}, UnusableResource{path/url, io}, UnknownConnectionType,
  ConflictingKeys }`.

## PersistentSentMessage (extended store usage — `truefix-store` / `truefix-session`)

The durable record enabling restart-survivable resend (FR-001/002).

- Keyed by outbound `MsgSeqNum`; value = the encoded wire bytes as sent.
- Written by `send_stored` when `PersistMessages != N`; read by the resend path.
- **State/relationship**: the store is the source of truth; the in-memory map is a write-through cache.
- **Rule** (FR-003): when `PersistMessages=N`, no body is stored ⇒ resend collapses the range to a
  SequenceReset-GapFill instead of replaying bodies.

## Structured Group / Component (extended — `truefix-core` / `truefix-dict`)

Repeating-group representation after dictionary-driven decode (FR-004/005).

- `Group { count_tag, delimiter_tag, entries: Vec<FieldMap> }`; entries may contain nested `Group`s.
- `Component` expands to its member fields/groups per the dictionary.
- **Validation rules**: declared count == `entries.len()`; first field of each entry == `delimiter_tag`
  when `FirstFieldInGroupIsDelimiter`; member order respected when `ValidateUnorderedGroupFields`.
- Each violation → a session-level Reject with the corresponding `RejectReason`.

## TypedMessage / FieldEnum / MessageCracker (new — generated + `truefix`)

Generated dual-track artifacts (FR-020/021/022).

- `TypedMessage` (per version/MsgType): a thin wrapper over the generic `Message` with typed field,
  group, and component accessors; encodes/decodes byte-identically with the generic path.
- `FieldEnum` (e.g. `Side { Buy, Sell, … }`): typed value enumerations from the dictionary's allowed
  values.
- `MessageCracker`: dispatches an incoming `Message` to a typed handler by `(BeginString, MsgType)`.
- **Invariant**: dual-track FNV-1a hash proves generated artifacts and runtime `DataDictionary` share one
  source.

## RejectionOutcome (new — `truefix` callback API)

Typed callback results (FR-016) — supersedes `Result<(), String>`.

- `Reject { reason: SessionRejectReason, ref_tag: Option<u32>, text: Option<String> }` — from
  `from_admin` (logon/admin refusal).
- `DoNotSend` — from `to_app` (suppress outbound; not stored as sent).
- `BusinessReject { reason: BusinessRejectReason, ref_tag: Option<u32>, text: Option<String> }` — from
  `from_app` (engine emits 35=j).

## Schedule (extended — `truefix-session`)

Adds weekly windows (FR-018) to the existing daily/weekday/UTC-offset schedule.

- New: `start_day: Option<Weekday>`, `end_day: Option<Weekday>`.
- `is_in_session(now)` handles intra-day and **cross-day weekly** windows.
- **Boundary behaviour** (state transition): crossing out of the window ⇒ `disconnect → reset seq →
  clear store`; crossing into the window (initiator) ⇒ `reconnect → reset`. `NonStopSession=Y` ⇒ no
  scheduled reset.

## TlsSpec (new — `truefix-config` / `truefix-transport`)

TLS material resolved from config (FR-017).

- `cert_path`, `key_path`, `ca_path: Option`, `min_version: Option<TlsVersion>`,
  `server_name: Option<String>` (SNI), `need_client_auth: bool`.
- **Rule**: `need_client_auth` ⇒ server requires & verifies a client cert against `ca_path` roots;
  missing/invalid client cert ⇒ handshake refused.

## ConnectEndpointSet (new — `truefix-transport` / `truefix-session`)

Ordered initiator targets for failover (FR-019).

- `endpoints: Vec<SocketAddr>` from numbered `SocketConnectHost<N>`/`SocketConnectPort<N>` keys.
- **Behaviour**: on reconnect, rotate to the next endpoint; exhaustion wraps per the reconnect policy
  without busy-looping/panicking.

## MetricsSignal (new — `truefix-transport`)

Exported observability measurements (FR-023).

- Gauges: `session_state`, `next_sender_seqnum`, `next_target_seqnum`.
- Counters: `messages_sent_total`, `messages_received_total`, `reconnects_total`.
- Labelled by `SessionID`; emitted via the `metrics` facade alongside `Monitor` updates.

## Appendix A Stance (extended — `truefix-config`)

Config-key registry (FR-027). Keys implemented by this feature move `Recognized → Implemented` in the
existing `APPENDIX_A_KEYS` registry; `key_coverage.rs` keeps SC-004/SC-014 accurate.
