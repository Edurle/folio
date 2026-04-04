# Global Mode Incremental Index Updates

**Date**: 2026-04-05

## Problem

Global mode (no `.folio/` directory) performs a full index rebuild on every CLI invocation. Workspace mode already uses incremental updates with `.folio/cache.json`. Global mode should also benefit from incremental updates.

## Design

### Cache Path Strategy

| Mode | Cache Path |
|------|-----------|
| Workspace | `<root>/.folio/cache.json` (unchanged) |
| Global | `~/.cache/folio/<sha256_prefix>/cache.json` |

- SHA-256 hash of the absolute root path, truncated to 16 hex characters for the directory name.
- Example: `/home/user/docs` → `~/.cache/folio/a1b2c3d4e5f6g7h8/cache.json`

### Changes

**File: `src/index/mod.rs`** — modify `build_index_incremental()`:

1. Remove the early return for non-workspace mode (lines 33-35).
2. Add cache path resolution logic:
   - Workspace mode → `<root>/.folio/cache.json` (existing behavior).
   - Global mode → `~/.cache/folio/<hash>/cache.json`.
3. Ensure cache directory exists before writing (create `~/.cache/folio/<hash>/` if needed).

**No other files need changes.** All commands already call `build_index_incremental()`.

### Fallback Behavior

- Cache load failure → full rebuild + save new cache (same as workspace mode today).
- Cache directory creation failure → fall back to full rebuild without caching (graceful degradation).

### Non-Goals

- Cache eviction/cleanup of stale entries under `~/.cache/folio/`.
- Any changes to workspace mode behavior.
