# Contract: truefix-session (Session layer)

Covers FR-D1…D12, FR-E1…E3, FR-K (auth/reject), parts of US1/US3/US6/US10/US12.

## Provided behavior

- **State machine**: explicit states & transitions for initiator and acceptor (Disconnected,
  Logon{Sent,Received}, LoggedOn, Logout{Sent,Received}, Reconnecting). Transitions driven by inbound
  admin messages, timers, and operator actions.
- **Sequence management**: next sender/target sequence numbers persisted via `MessageStore`; high-seq →
  ResendRequest + queue; low-seq w/o PossDup → disconnect (FR-D12).
- **Admin messages**: generate/handle Logon, Logout, Heartbeat, TestRequest, ResendRequest,
  SequenceReset (GapFill & Reset), Reject, BusinessMessageReject per version.
- **Handshake & reset**: Logon/Logout; ResetSeqNumFlag(141); ResetOn{Logon,Logout,Disconnect};
  RefreshOnLogon (FR-D4/D5).
- **Liveness**: Heartbeat at HeartBtInt; TestRequest on idle; `HeartBeatTimeoutMultiplier`,
  `TestRequestDelayMultiplier`, `DisableHeartBeatCheck` (FR-D6).
- **Recovery/resend**: ResendRequest with `ResendRequestChunkSize` (0 = unbounded); gap fill; app messages
  resent PossDupFlag=Y + OrigSendingTime; admin → SequenceReset-GapFill (FR-D7…D9).
- **789 sync**: `NextExpectedMsgSeqNum` logon synchronization for FIX ≥ 4.4;
  `EnableLastMsgSeqNumProcessed` (369) (FR-D10).
- **Timeouts/latency**: Logon/Logout timeouts; `CheckLatency`/`MaxLatency` SendingTime validation (FR-D11).
- **Scheduling**: in-session windows (StartTime/EndTime, StartDay/EndDay, Weekdays, TimeZone),
  NonStopSession; scheduled reset = disconnect→reset seq→clear store→reconnect (FR-E1…E3).
- **Auth/reject**: Logon Username/Password; custom auth via to/fromAdmin; session-level Reject and
  Business Message Reject; `RejectInvalidMessage`, `RejectMessageOnUnhandledException` (FR-K1/K2).

## Acceptance hooks
- Table-driven transition tests; two-process integration for handshake/heartbeat/resend.
- Server AT sequence/logon/PossDup scenarios (Appendix B) drive this crate.
