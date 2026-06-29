# Feature-Parity Requirements Checklist: TrueFix (QuickFIX/J Parity Baseline)

**Purpose**: "Unit tests for English" — validate that the requirements covering the full config-key set
(Appendix A) and the full AT-case set (Appendix B) are complete, clear, consistent, and measurable, so
this checklist can serve as the parity-coverage ledger for `/speckit-implement` and `/speckit-analyze`.
**Created**: 2026-06-29
**Feature**: [spec.md](../spec.md)

**How to use**: Each item tests whether the *requirement* is well-specified, not whether code works. An
item is checkable when the spec (or downstream plan/tasks) defines the named behavior unambiguously with
a verifiable acceptance criterion. `[Gap]` items flag requirements that must be added or explicitly
deferred-with-reason before they can be checked.

## Config-Key Coverage — Completeness (Appendix A)

- [ ] CHK001 - Is every Appendix A config key explicitly classified as implemented OR documented
  unsupported-with-reason, with no key left silently unaddressed? [Completeness, Spec §FR-I2 / Appendix A]
- [ ] CHK002 - Are requirements defined for the **session identity & type** keys (BeginString,
  Sender/Target Comp/Sub/Location IDs, SessionQualifier, ConnectionType, Description, DefaultApplVerID)?
  [Completeness, Spec §FR-D2 / Appendix A]
- [ ] CHK003 - Are requirements defined for every **dictionary & validation** key (UseDataDictionary,
  Data/App/TransportDataDictionary, the Validate* toggles, AllowUnknownMsgFields, CheckLatency,
  MaxLatency, CheckCompID, RejectGarbledMessage, RejectInvalidMessage,
  RejectMessageOnUnhandledException, FirstFieldInGroupIsDelimiter)? [Completeness, Spec §FR-C3 / Appendix A]
- [ ] CHK004 - Are requirements defined for every **session behavior** key (HeartBtInt & multipliers,
  Logon/Logout timeouts, the Reset*/Refresh keys, PersistMessages, ResendRequestChunkSize,
  SendRedundantResendRequests, ClosedResendInterval, Enable{LastMsgSeqNumProcessed,NextExpectedMsgSeqNum},
  RequiresOrigSendingTime, AllowPosDup, ForceResendWhenCorruptedStore, DisconnectOnError,
  TimeStampPrecision, MaxScheduledWriteRequests, ContinueInitializationOnError,
  LogMessageWhenSessionNotFound, LogonTag)? [Completeness, Spec §FR-D / Appendix A]
- [ ] CHK005 - Are requirements defined for all **scheduling** keys (StartTime/EndTime, StartDay/EndDay,
  Weekdays, TimeZone, NonStopSession)? [Completeness, Spec §FR-E1/E2 / Appendix A]
- [ ] CHK006 - Are requirements defined for the **acceptor / dynamic-session** keys (SocketAccept*,
  AcceptorTemplate, DynamicSession, AllowedRemoteAddresses)? [Completeness, Spec §FR-F3 / Appendix A]
- [ ] CHK007 - Are requirements defined for the **initiator** keys (SocketConnect*, SocketLocal*,
  ReconnectInterval)? [Completeness, Spec §FR-F4/F5 / Appendix A]
- [ ] CHK008 - Are requirements defined for the **socket options** keys (KeepAlive, TcpNoDelay,
  ReuseAddress, Linger, OobInline, buffer sizes, TrafficClass, SynchronousWrites/Timeout)?
  [Completeness, Spec §FR-F5 / Appendix A]
- [ ] CHK009 - Are requirements defined for the full **SSL/TLS** key-set (UseSSL, EnabledProtocols,
  CipherSuites, key/trust store + types + passwords, factory algorithms, NeedClientAuth,
  EndpointIdentificationAlgorithm, UseSNI, SNIHostName)? [Completeness, Spec §FR-F6 / Appendix A]
- [ ] CHK010 - Are requirements defined or an explicit unsupported-with-reason stance recorded for the
  **proxy** keys (ProxyType/Version/Host/Port/User/Password/Domain/Workstation)? [Coverage, Spec §FR-I2 / Appendix A]
- [ ] CHK011 - Are requirements defined for the **store backend** keys (FileStore*, and the SQL/JDBC
  store keys) and for the screen/file/facade **log** keys? [Completeness, Spec §FR-G/H / Appendix A]
- [ ] CHK012 - Is the **Sleepycat/JE** key group (SleepycatDatabaseDir/MessageDbName/SequenceDbName)
  explicitly marked deferred-with-reason for v1 rather than ambiguous? [Consistency, Spec §FR-G1 / Clarifications 2026-06-29]
- [ ] CHK013 - Are requirements for `[DEFAULT]`-vs-`[SESSION]` inheritance/override precedence and
  `${name}` interpolation specified precisely enough to verify? [Clarity, Spec §FR-I1/I3]

## Config-Key Coverage — Clarity & Measurability

- [ ] CHK014 - For each validation toggle, is the *accept/reject outcome change* specified individually
  (on vs off), rather than only the key being listed? [Clarity, Spec §FR-C3]
- [ ] CHK015 - Is the behavioral distinction between the **two rejection layers** (dictionary/validation
  failure vs FIX protocol-level basic-validity failure) defined unambiguously? [Clarity, Spec §FR-C4]
- [ ] CHK016 - Are value domains/types specified for keys whose semantics depend on them (e.g.
  TimeStampPrecision enum, ResendRequestChunkSize 0=unbounded, TimeZone format, Weekdays format)?
  [Clarity, Spec §FR-D7/E1 / Appendix A]
- [ ] CHK017 - Is the meaning of "implemented" vs "documented unsupported-with-reason" measurable (i.e.
  is there an objective check that distinguishes the two states per key)? [Measurability, Spec §SC-004]
- [ ] CHK018 - Are CheckLatency/MaxLatency requirements quantified (units, comparison direction,
  reject behavior) rather than left as "validate latency"? [Clarity, Spec §FR-D11 / Appendix A]

## AT-Case Coverage — Completeness (Appendix B)

- [ ] CHK019 - Is the complete set of 73 distinct server-suite AT scenarios enumerated as the porting
  baseline, with none omitted from the parity list? [Completeness, Spec §FR-M1 / Appendix B]
- [ ] CHK020 - Are requirements defined for the **logon-category** AT scenarios (1a–1e: valid logon,
  MsgSeqNum too high, duplicate identity, invalid sender/target, bad sending time, length invalid,
  no DefaultApplVerID, wrong BeginString, not-a-logon)? [Completeness, Spec §FR-D4 / Appendix B]
- [ ] CHK021 - Are requirements defined for the **sequence-handling** AT scenarios (2a–2c, 10_*, 11a–11c
  SequenceReset NewSeqNo greater/equal/less)? [Completeness, Spec §FR-D8/D12 / Appendix B]
- [ ] CHK022 - Are requirements defined for the **PossDup/PossResend** AT scenarios (2e–2g, 19a/19b)?
  [Completeness, Spec §FR-D9 / Appendix B]
- [ ] CHK023 - Are requirements defined for the **message-validity / reject** AT scenarios (2i–2t,
  3b/3c, 14a–14j, 15, 21, 7_ReceiveRejectMessage)? [Completeness, Spec §FR-C4/K2 / Appendix B]
- [ ] CHK024 - Are requirements defined for the **heartbeat / test-request** AT scenarios
  (4a, 4b, 6)? [Completeness, Spec §FR-D6 / Appendix B]
- [ ] CHK025 - Are requirements defined for the **admin/app routing** AT scenarios (8_* variants incl.
  FIX50SP2, ReverseRoute, ReverseRouteWithEmptyRoutingTags)? [Completeness, Spec §FR-J / Appendix B]
- [ ] CHK026 - Are requirements defined for the named **regression** AT scenarios (QFJ634, QFJ648,
  QFJ650, QFJ934, AlreadyLoggedOn, RejectResentMessage, SessionReset, MinQty40–50,
  LogonUnknownDefaultApplVerID, 13b, 20_SimultaneousResendRequest)? [Completeness, Spec §FR-M1 / Appendix B]
- [ ] CHK027 - Are requirements defined for each **special-category** AT suite (nextExpectedMsgSeqNum,
  lastMsgSeqNumProcessed, resendRequestChunkSize, validateChecksum, rejectGarbledMessages, timestamps,
  resynch)? [Completeness, Spec §FR-D7/D10 / Appendix B]
- [ ] CHK028 - Is the per-version applicability of each AT scenario specified (which scenarios target
  which of fix40–fixLatest), rather than assumed uniform? [Coverage, Spec §FR-M2 / Appendix B]

## AT-Case Coverage — Clarity, Measurability & Consistency

- [ ] CHK029 - Is "passing an AT scenario" defined as an objective match of sent-messages + disconnect
  behavior against the scenario's expected steps (measurable pass/fail)? [Measurability, Spec §FR-M1/US12]
- [ ] CHK030 - Is the release gate unambiguous that **all** targeted versions must pass, with the only
  exception being explicitly-listed per-scenario deferrals? [Clarity, Spec §FR-M3 / Clarifications]
- [ ] CHK031 - Do the AT requirements and the session-layer FRs (§FR-D*) agree on behavior for the same
  condition (e.g. MsgSeqNum-too-high handling in 2b vs §FR-D12)? [Consistency, Spec §FR-D12 / Appendix B]
- [ ] CHK032 - Is the black-box porting constraint (reproduce behavior via independently-authored
  fixtures; no copied source/runner) stated clearly enough to audit for License compliance?
  [Clarity, Spec §FR-M1 / Constitution III]

## Cross-Cutting Consistency & Traceability

- [ ] CHK033 - Does every Appendix A key map to at least one domain requirement (A–N), with no orphan
  keys lacking an owning FR? [Consistency, Spec §Appendix A]
- [ ] CHK034 - Are FIX-version scope statements consistent across spec (§FR-A1 lists 4.0–5.0SP2+FIXT;
  Appendix B versions; §FR-M3 gate) with no version silently dropped or added? [Consistency, Spec §FR-A1/M3]
- [ ] CHK035 - Is terminology canonical and non-conflicting across config keys, FRs, and AT references
  (e.g. ResetSeqNumFlag, SequenceReset-GapFill, OrigSendingTime used consistently)? [Consistency]
- [ ] CHK036 - Is a stable ID scheme present so each config key and AT scenario is individually
  traceable from spec → plan → tasks → implement coverage? [Traceability, Gap]

## Scenario-Class & Edge-Case Coverage

- [ ] CHK037 - Are **recovery** requirements specified (message recovery after gap, resend across
  process restart with persistent store, ForceResendWhenCorruptedStore)? [Coverage, Spec §FR-D12/G2/SC-006]
- [ ] CHK038 - Are **exception/error** requirements specified for garbled/truncated input and the
  RejectGarbledMessage toggle, including the no-panic guarantee on the parse path? [Coverage, Spec §FR-B8/C4]
- [ ] CHK039 - Are **scheduling-boundary** requirements specified for the reset semantics (disconnect →
  reset seq → clear store → reconnect) and NonStop exclusion? [Coverage, Spec §FR-E3 / Appendix B SessionReset]
- [ ] CHK040 - Are **sub-second timestamp** edge requirements specified (accept ≤ picosecond, store
  truncated to nanosecond) consistently between codec FR and the timestamps AT suite? [Edge Case, Spec §FR-B7 / Appendix B]
- [ ] CHK041 - Are **acceptor-specific** edge requirements specified at parity with initiator (dynamic
  session from disallowed address, multi-session routing by SessionID)? [Coverage, Spec §FR-F1/F3 / Constitution VI]

## Dependencies, Assumptions & Non-Functional Linkage

- [ ] CHK042 - Is the dictionary-provenance assumption (FIX Orchestra/Repository → normalized format,
  single source for codegen + runtime) documented and validated as License-safe? [Assumption, Spec §FR-A2 / Clarifications]
- [ ] CHK043 - Are the "JDBC-equivalent" and "JMX-equivalent" parity substitutions documented as
  capability-parity (not literal-tech) so coverage can be judged objectively? [Assumption, Spec §Assumptions]
- [ ] CHK044 - Are non-functional acceptance items (no-panic critical paths, typed errors, structured
  observability, benchmarks-for-visibility-not-gated) tied to verifiable success criteria? [Measurability, Spec §SC-005/007/008]

## Notes

- Check items off as their underlying requirement is confirmed complete/clear/consistent/measurable.
- `[Gap]`/`[Assumption]` items (e.g. CHK036) may require adding a requirement or an explicit
  deferred-with-reason note before they can be checked.
- This checklist is the requirements-quality gate for the parity baseline; the concrete per-key and
  per-scenario *implementation* coverage is tracked separately in tasks.md once `/speckit-tasks` runs.
- Traceability: 43/44 items carry a spec §/Appendix/Constitution reference or an explicit marker
  (CHK036 is itself the traceability-scheme gap item).
