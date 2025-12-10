# blusc


This is an attempt at a pure Rust implementation of Blosc compression and decompression.


🚧 work in progress 🚧


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

## Development

```sh
cargo build
```

### Testing

```sh
cargo test -- --test-threads=1 --nocapture
```



For reference during development, this crate contains the C implementations in `c-blosc` and `c-blosc2` directories as git submodules.

-----

Motivation: In 2025, why are all of our compression algorithms still bindings to C libraries?