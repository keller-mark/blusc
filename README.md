# blusc


This is an attempt at a pure Rust implementation of [c-blosc2](https://github.com/Blosc/c-blosc2) compression and decompression.


Blusc is not intended to be as performant as the reference C implementation, as the goal here is to enable easy compilation to WASM targets (so we avoid optimizations like multi-threading, hardware acceleration, etc).

## Background

The Blosc algorithm wraps other compression methods, including zstd, snappy, lz4, and zlib. We will use pure Rust implementations of these inner compression methods:

- ruzstd crate for zstd: https://github.com/KillingSpark/zstd-rs
- snap crate for snappy: https://github.com/BurntSushi/rust-snappy
- lz4_flex crate for lz4: https://github.com/pseitz/lz4_flex
- flate2 crate for zlib: https://github.com/rust-lang/flate2-rs

This crate offers pure rust implementations of the following functions and constants:

- blosc1_cbuffer_metainfo
- blosc1_cbuffer_validate
- blosc1_cbuffer_sizes
- blosc1_getitem
- blosc2_get_complib_info
- blosc2_compress
- blosc2_decompress
- BLOSC_NOSHUFFLE
- BLOSC2_MAX_OVERHEAD
- BLOSC2_CPARAMS_DEFAULTS
- BLOSC2_DPARAMS_DEFAULTS
- blosc2_create_cctx
- blosc2_compress_ctx
- blosc2_cbuffer_sizes
- blosc2_create_dctx
- blosc2_decompress_ctx


<!-- Motivation: Why are all of our compression algorithms still bindings to C libraries? -->

## Development

```sh
cargo build
```

### Testing

```sh
cargo test -- --test-threads=1 --nocapture
```



For reference during development, this repository contains the C implementations in `c-blosc` and `c-blosc2` directories as git submodules.


## AI usage disclaimer

This README, `Cargo.toml`, the public interface/function signatures, initial unit tests, and the contents of `src/internal/constants.rs` were written by me, a human, or manually ported.
The rest of the Rust code and other unit tests were largely LLM-generated/ported (see `AGENTS.md` for more details).


## Citing Blosc

Copied from the [citing-blosc](https://github.com/Blosc/c-blosc2/tree/main?tab=readme-ov-file#citing-blosc) section of the c-blosc2 README.

```bibtex
@ONLINE{blosc,
    author = {{Blosc Development Team}},
    title = "{A fast, compressed and persistent data store library}",
    year = {2009-2025},
    note = {https://blosc.org}
}
```

## Related work

WASM compilation of c-blosc in [numcodecs.js](https://github.com/manzt/numcodecs.js/tree/main/codecs/blosc) via Emscripten.
