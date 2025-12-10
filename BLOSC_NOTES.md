# My current understanding of the BLOSC algorithm

## Compression

## Decompression

## Extended Header Flags

In Blosc2, if the header is an extended header (32 bytes), the `flags` byte (byte 2) may have both `BLOSC_DOSHUFFLE` (0x1) and `BLOSC_DOBITSHUFFLE` (0x2) set. This combination (0x3) is invalid for actual filtering (you can't do both byte shuffle and bit shuffle at the same time) and serves as a marker for the extended header.

When this marker is present, the actual shuffle settings should be read from the `filters` array in the extended header (bytes 16-21).

- `filters[i] == 1` (BLOSC_SHUFFLE) -> Do byte shuffle.
- `filters[i] == 2` (BLOSC_BITSHUFFLE) -> Do bit shuffle.

The `decompress` function in `src/internal/mod.rs` has been updated to handle this: if both flags are set in the header, it ignores them and checks the `filters` array instead.