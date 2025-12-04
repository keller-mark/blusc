# Plan: Blosc2 Rust Implementation.

This plan prioritizes the creation of a pure Rust implementation of Blosc compression and decompression functions, with an API to enable drop-in replacement for the functions listed in the `README.md`.
This plan focuses on in-memory operations and avoids SIMD optimizations for the initial implementation.

## Steps

1.  **Establish Core Structure API Foundation**
    Create a `src/api.rs` file to house the exported functions. Define structs and constants here. Set up to organize the project into modules (`api`, `internal`, `codecs`, `filters`).

2.  **Port `blosclz` and Generic Filters**
    In new modules `src/codecs/blosclz.rs` and `src/filters/mod.rs`, port the `blosclz` compressor and the generic, non-SIMD versions of the shuffle and bitshuffle filters from C to pure Rust.

3.  **Implement Core Compression Logic**
    In a new `src/internal/mod.rs`, develop the core Rust functions that will be called by the API wrappers. These functions will orchestrate applying filters, calling codecs, and managing the Blosc header. These functions will call the pure rust implementations of the zstd, snappy, lz4, and zlib compression algorithms using the crates identified in the `README.md`

4.  **Add Compatibility Testing**
    Create integration tests in the  directory that call the Rust functions and compare their output against the original Blosc2 library to ensure correctness and drop-in compatibility.

## Further Considerations

1.  **API Exposure**: The C-like API will be the primary public interface to enable drop-in replacement. The internal Rust functions can be developed with an eye toward making a more idiomatic Rust API public in the future.
2.  **Error Handling**: The C-like API will return integer error codes, as is standard. The internal Rust functions should use `Result` and the C-like API wrappers will be responsible for translating these into error codes.