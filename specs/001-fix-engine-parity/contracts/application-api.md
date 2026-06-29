# Contract: truefix (Facade — Application & MessageCracker)

Covers FR-J1/J2. Async-first integrator surface (FR-F7).

## Application (trait) — async callbacks
- `on_create(session_id)` — session constructed.
- `on_logon(session_id)` / `on_logout(session_id)` — session up/down.
- `to_admin(msg, session_id)` — outbound admin message hook (e.g. add Username/Password to Logon).
- `from_admin(msg, session_id) -> Result<(), Reject>` — inbound admin hook (e.g. validate credentials).
- `to_app(msg, session_id) -> Result<(), DoNotSend>` — outbound app message hook.
- `from_app(msg, session_id) -> Result<(), BusinessReject>` — inbound app message hook.
- All callbacks are `async`; the engine drives them on the tokio runtime.

## MessageCracker
- Dispatches a typed message to a per-message-type handler; configurable fallback for unhandled types
  (FR-J2). Works with the codegen typed messages from `truefix-dict`.

## Facade
- `truefix` re-exports the public surface of the layer crates so integrators depend on one crate.
- Stable, documented public API (Constitution I); breaking changes follow semver with migration notes.

## Acceptance hooks
- Test Application asserting callback order across create→logon→admin/app exchange→logout.
- Cracker routes a NewOrderSingle to its typed handler; unhandled type hits the fallback.
