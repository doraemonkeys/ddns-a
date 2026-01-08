# Implementation Plan

> No pending tasks. All previous implementations complete.

## ✅ Completed: Filter 重构 (Filter Refactoring)

**Status**: Complete (2026-01-08)  
**Coverage**: 91.14% (target: ≥90%)

### Summary

Refactored the adapter filtering system to fix the multi-include AND semantics bug and provide a symmetric API for kind-based filtering:

| Phase | Description | Status |
|-------|-------------|--------|
| Phase 1 | Core Filter refactoring (`filter.rs`) - `KindFilter`, `FilterChain`, `NameRegexFilter` | ✅ |
| Phase 2 | CLI changes (`cli.rs`) - `AdapterKindArg`, `--include-kind`, `--exclude-kind` | ✅ |
| Phase 3 | TOML changes (`toml.rs`) - `include_kinds`, `exclude_kinds` | ✅ |
| Phase 4 | Validated config (`validated.rs`) - `FilterChain`, loopback default | ✅ |
| Phase 5 | Integration & cleanup - CI passes | ✅ |

### Key Changes

- **New**: `KindFilter` - pure matcher by adapter kind
- **New**: `FilterChain` - correct Include OR / Exclude AND semantics
- **Updated**: `NameRegexFilter` - pure matcher, no mode
- **Removed**: `CompositeFilter`, `ExcludeVirtualFilter`, `ExcludeLoopbackFilter`, `FilterMode`
- **CLI**: `--include-kind`, `--exclude-kind` replace `--exclude-virtual`
- **Behavior**: Loopback excluded by default unless explicitly included
