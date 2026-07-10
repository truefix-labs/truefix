# Data Model

| Entity | Required fields | Rules |
|---|---|---|
| BaselineOperation | source identity, method, path, auth, replay class, entrypoint, fixture | One unique record per Python operation; all 264 classified. |
| CanonicalRequest | timestamp, method, path/query, headers, body | Timestamp is UTC milliseconds; empty query values omitted; bytes signed and sent identically. |
| ParityFinding | baseline identity, observed behavior, disposition, evidence | Unsupported server endpoint is removed or explicitly classified as a convenience abstraction. |
| RetryDecision | request safety, failure class, attempt count | Read only; at most one retry; writes never retry. |
