# Dictionary source pipeline (stub — Stage S0)

This directory holds the **normalized TrueFix dictionary** inputs and the transform that produces them.
Per Constitution Principle III and spec FR-A2, dictionaries are **derived from the FIX Trading
Community's official machine-readable specifications (FIX Orchestra / Repository)** and normalized into
TrueFix's own format. QuickFIX/J `FIXxx.xml` files are **never** copied.

## Layout (to be populated in Stages S4/S7)

```
dict-src/
├── orchestra/      # vendored FIX Orchestra/Repository inputs (official, license-checked)
├── normalized/     # generated TrueFix normalized dictionaries (one per version)
└── generate.rs     # transform: orchestra -> normalized (invoked from build.rs)
```

## Dual-track usage (Principle IV)

The single `normalized/` output is consumed by:
- `build.rs` codegen → strongly-typed per-version messages (compile-time).
- the runtime `DataDictionary` validator (run-time).

A build assertion (T047) verifies both originate from the same normalized version/hash so they cannot
diverge.

## Status

S0 scaffold only. Population of `orchestra/` + the `generate` transform is tracked by tasks T048, T050,
T074, T075.
