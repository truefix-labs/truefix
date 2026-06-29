# Reference wire vectors (T099)

Byte-exact reference output captured from **QuickFIX/J** for the canonical and per-version message set,
used by the round-trip / BodyLength / CheckSum tests (T010, T072; SC-002).

## License-safe capture (Constitution Principle III)

Only **output bytes** are recorded here — the serialized FIX wire form QuickFIX/J produces for a given
logical message. No QuickFIX/J source code, test code, or private data files are copied.

## Format (planned)

Each fixture is a `<name>.fix` file containing the SOH-delimited bytes (SOH shown as `|` in any
human-readable companion `.txt`), plus a `<name>.json` describing the logical message that produced it,
so the test can build the same message in TrueFix and assert byte-identical output.

## Status

S0 scaffold (directory + procedure). Actual capture requires building QuickFIX/J and recording its
output; that step is pending and keeps T099 open until the vectors are committed here. Canonical set:
NewOrderSingle, ExecutionReport, Logon, Heartbeat, ResendRequest, SequenceReset, Reject — across
FIX 4.0–5.0SP2 + FIXT 1.1.
