# Dictionary source pipeline

This directory holds the **normalized TrueFix dictionary** inputs and the transform that produces them.
Per Constitution Principle III and spec FR-A2, dictionaries are **derived from the FIX Trading
Community's official machine-readable specifications (FIX Orchestra / Repository)** and normalized into
TrueFix's own format. QuickFIX/J `FIXxx.xml` files are **never** copied.

## Current layout

```
dict-src/
├── fix-repository/ # provenance for classic FIX Repository-derived inputs
├── orchestra/      # FIX Orchestra input used by the conversion tests/tooling
└── normalized/     # checked-in `.fixdict` dictionaries consumed by runtime and codegen
```

## Dual-track usage (Principle IV)

The single `normalized/` output is consumed by:
- `build.rs` codegen → strongly-typed per-version messages (compile-time).
- the runtime `DataDictionary` validator (run-time).

A build assertion verifies that runtime and generated views originate from the same normalized
content hash.

## Current contents

The repository currently contains normalized dictionaries for FIX 4.0–4.4, FIXT 1.1, FIX 5.0,
FIX 5.0 SP1/SP2, and FIX Latest, plus a FIX Latest Orchestra XML input. This directory is source
data, not a standalone crate; run `cargo test -p truefix-dict --features dict-tooling` to validate
parsing, conversion, hashes, and code generation.
