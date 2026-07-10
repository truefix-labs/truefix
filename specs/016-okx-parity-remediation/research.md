# Research: OKX Parity Remediation

## Decisions

- **Baseline scope**: Use `python-okx@fa8d738` to enumerate all 264 operations. **Rationale**: measurable parity. **Alternative**: repair only listed findings; rejected because it leaves unclassified gaps.
- **Protocol authority**: Use official OKX documentation for path, verb, headers, signing, and response details. **Rationale**: Python can contain historical defects. **Alternative**: mirror Python exactly; rejected for protocol risk.
- **Retries**: Retry a safe read at most once after transient failure; never replay writes. **Rationale**: bounded recovery without duplicate asset changes.
- **Non-baseline operations**: Remove service endpoints absent from Python; retain only pure convenience mappings. **Rationale**: no unverified server capabilities.
