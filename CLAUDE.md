<!-- SPECKIT START -->
For additional context about technologies to be used, project structure,
shell commands, and other important information, read the current plan:
`specs/001-fix-engine-parity/plan.md`

Active feature: **001-fix-engine-parity** — full QuickFIX/J-parity production FIX engine in Rust.
Key context: async-first (tokio), TLS via rustls, typed errors (thiserror), rust_decimal, dual-track
data dictionary (build.rs codegen + runtime DataDictionary from a normalized FIX Orchestra source),
layered cargo workspace (truefix-core/dict/session/store/log/transport/config + truefix facade +
truefix-at). Spec: `specs/001-fix-engine-parity/spec.md` (Appendix A config keys, Appendix B AT cases).
Governing constitution: `.specify/memory/constitution.md` (v2.0.0). One-pass full parity, internally
staged S0–S9 with per-stage checkpoints; AT suite is the release gate.
<!-- SPECKIT END -->
