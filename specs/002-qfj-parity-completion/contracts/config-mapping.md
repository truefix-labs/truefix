# Contract: Settings → Engine Mapping (US1; FR-013/014/015)

## Surface

- `EngineConfig::from_settings(&Settings) -> Result<EngineConfig, ConfigError>` (`truefix-config`).
- `Engine::start(settings) -> Result<EngineHandle, ConfigError>` (`truefix` facade) — builds and starts
  every configured session, routing by `ConnectionType`.

## Behaviour

1. **Resolution & precedence**: `[DEFAULT]` values apply to every session; a key present in a `[SESSION]`
   overrides the DEFAULT for that session. Output is a typed `ResolvedSessionConfig` per session
   (see data-model).
2. **ConnectionType routing**: `acceptor` ⇒ acceptor construction; `initiator` ⇒ initiator construction.
   An unknown/missing `ConnectionType` ⇒ `ConfigError{ kind: UnknownConnectionType }`.
3. **Fail-fast, all-or-nothing**: the first invalid value, missing required key, or unusable referenced
   resource (store dir, log dir, TLS path, SQL URL) ⇒ `Err(ConfigError{ key, session, kind })`. On error,
   **no session is started** and no partially-initialised engine is left running.
4. **Coverage**: store/log/schedule/transport/TLS/socket all take effect from config alone (no code-level
   construction required for the standard cases).

## Acceptance (maps to spec US1 scenarios)

- Acceptor-only `.cfg` ⇒ acceptor accepts a counterparty logon. ✔
- Initiator `.cfg` to a reachable acceptor ⇒ connects, logs on, heartbeats. ✔
- Malformed recognised-key value ⇒ typed error naming key + session; nothing started. ✔
- DEFAULT overridden per SESSION ⇒ per-session value wins. ✔

## Errors

`ConfigError{ key, session, kind ∈ {MissingRequired, InvalidValue, UnusableResource, UnknownConnectionType,
ConflictingKeys} }`. Never panics; always typed (Principle I).

## Test hooks

Two-process integration: start engine from a fixture `.cfg` with one acceptor + one initiator; assert a
completed logon→heartbeat. Negative tests assert the exact `ConfigError` variant + key/session.
