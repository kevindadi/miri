# Petri Net Monitor for Miri

This document describes the Colored Petri Net (CPN) monitor integrated into Miri for protocol-level concurrency checking.

## Overview

The Petri monitor observes protocol-layer events during Miri execution (lock/unlock, atomic load/store, thread spawn/join, block/wake) and maps them to transitions in a user-defined Colored Petri Net. If an event corresponds to a transition that is not enabled in the current marking, a **protocol violation** is reported.

## Usage

### Running with Custom-Built Miri (no rustup/cargo-miri)

If you build miri from source (e.g. `cargo build` in this repo or via the rust tree), use **`cargo test`** instead of `cargo miri test`. The ui test harness invokes your built miri binary directly.

1. **Build miri**: `cargo build`
2. **Set MIRI_SYSROOT** to a rust sysroot. Options:
   - Nightly: `export MIRI_SYSROOT=$(rustup run nightly rustc --print sysroot)`
   - Custom toolchain (e.g. `miri`): `export MIRI_SYSROOT=$(rustup run miri rustc --print sysroot)`
   - Or a path like `~/.rustup/toolchains/nightly-<host>/`
3. **Run the petri test**:
   ```bash
   MIRIFLAGS="-Zmiri-petri=petri_config.json" cargo test tests/petri/mutex_violation
   ```

### Running with rustup-installed Miri

If you use `rustup component add miri`, run:

```bash
MIRIFLAGS="-Zmiri-petri=petri_config.json" cargo +nightly miri test tests/petri/mutex_violation
```

### Enabling the Monitor

```bash
MIRIFLAGS="-Zmiri-petri=path/to/petri_config.json" cargo test <test_name>
# or with cargo miri:
MIRIFLAGS="-Zmiri-petri=path/to/petri_config.json" cargo +nightly miri test <test_name>
```

Or with additional options:

```bash
MIRIFLAGS="-Zmiri-petri=petri_config.json -Zmiri-petri-log=petri.ndjson -Zmiri-petri-print-marking-on-each-event" cargo miri test
```

### Command-Line Flags

| Flag | Description |
|------|-------------|
| `-Zmiri-petri=<path>` | Enable the Petri monitor with the given JSON config file |
| `-Zmiri-petri-log=<path>` | Append events and marking hashes to the specified file (NDJSON) |
| `-Zmiri-petri-fail-fast` | Abort immediately on violation (default) |
| `-Zmiri-petri-no-fail-fast` | Log violations but continue execution |
| `-Zmiri-petri-print-marking-on-each-event` | Print marking hash after each event (debugging) |

### GenMC Mode

With GenMC enabled, the monitor records the marking hash at the end of each explored execution. When `-Zmiri-petri-log` is set, each execution appends a line like `{"exec_end": true, "marking_hash": 12345}`.

```bash
MIRIFLAGS="-Zmiri-genmc -Zmiri-petri=petri_config.json -Zmiri-petri-log=petri.ndjson" cargo miri test
```

## Configuration Format

A minimal `petri_config.json` for a Mutex model:

```json
{
  "places": ["free", "held"],
  "transitions": {
    "acquire": {
      "pre": [{"place": "free", "variable": "L"}],
      "post": [{"place": "held", "variable": "L"}]
    },
    "release": {
      "pre": [{"place": "held", "variable": "L"}],
      "post": [{"place": "free", "variable": "L"}]
    }
  },
  "event_mapping": {
    "LockAcquire": "acquire",
    "LockRelease": "release"
  },
  "initial_marking": {
    "free": []
  }
}
```

- **places**: List of place names (optional, for documentation)
- **transitions**: Each transition has `pre` and `post` arcs. Each arc has `place` and either `variable` (bound from the event) or `kind`/`value` for concrete tokens
- **event_mapping**: Maps event type names to transition IDs
- **initial_marking**: Place -> array of tokens. Each token is `["Kind", value]` e.g. `["Lock", 0]`. Can be empty; lock tokens are added lazily when first seen

### Arc Token Patterns

- `{"place": "free", "variable": "L"}`: Variable `L` is bound from the event (e.g. `lock_id` for LockAcquire)
- `{"place": "p", "kind": "Lock", "value": 42}`: Concrete token

### Supported Events

- `ThreadSpawn`, `ThreadJoin`, `Yield`, `Block`, `Wake`
- `LockAcquire`, `LockRelease`
- `AtomicLoad`, `AtomicStore`

## Example: Running the Test

**Custom-built miri** (this repo):
```bash
cd /path/to/miri
cargo build
export MIRI_SYSROOT=$(rustup run nightly rustc --print sysroot)   # or your rust sysroot path
MIRIFLAGS="-Zmiri-petri=petri_config.json" cargo test tests/petri/mutex_violation
```

**rustup miri**:
```bash
MIRIFLAGS="-Zmiri-petri=petri_config.json" cargo +nightly miri test tests/petri/mutex_violation
```

**Miri as rust submodule**: use `./x.py test src/tools/miri` from the rust tree.

**Lib-only** (no interpreter, just petri unit tests):
```bash
cargo test --lib petri
```

## Implementation Notes

- **Dynamic identity**: Lock and location IDs use runtime object addresses (e.g. `Rc::as_ptr` for MutexRef, `ptr.addr().bytes()` for atomic locations)
- **No weak memory**: The Petri net abstracts resource/protocol state; memory model details are handled by Miri/GenMC
- **Modular hooks**: Events are emitted from `intrinsics/atomic.rs`, `concurrency/thread.rs`, `concurrency/sync.rs`, and `concurrency/genmc/shims.rs`
