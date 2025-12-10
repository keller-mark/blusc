# My current understanding of the BLOSC algorithm

## Compression

### Block Splitting

Reference: `c-blosc2/blosc/stune.c` lines 185-215

The `split_block()` function determines whether to split a block into multiple streams (one per typesize byte). Splitting is only done for:
- Fast codecs: BLOSCLZ, LZ4, or ZSTD with clevel <= 5
- When byte-shuffle (BLOSC_SHUFFLE) is enabled (NOT bitshuffle!)
- When typesize <= MAX_STREAMS (16)
- When blocksize / typesize >= BLOSC_MIN_BUFFERSIZE

**Important**: Bitshuffle (BLOSC_BITSHUFFLE) does NOT enable block splitting. Only byte-shuffle does.

### BLOSClz Compression

Reference: `c-blosc2/blosc/blosclz.c` lines 422-650

The BLOSClz compressor uses several key optimizations:

1. **ipshift and minlen**: Set to 4 for optimal compression with bitshuffle and small typesizes
   - `ipshift = 4`: Shifts back in the match by 4 bytes to find longer runs
   - `minlen = 4`: Minimum match length to encode

2. **Match Finding**: Uses hash table to find potential matches
   - Hash function: `(seq * 2654435761) >> (32 - hashlog)`
   - Matches must be at least 4 bytes initially

3. **Match Length Calculation**: Critical bug was in the Rust implementation
   - C: `ip = get_run_or_match(ip, ip_bound, ref, !distance)` returns pointer AFTER last match byte
   - C: `ip -= ipshift` to bias the position
   - C: `len = ip - anchor` gives biased match length
   - **Bug in Rust**: Was checking `ref_ptr < ip_limit` which prevented finding long matches
   - **Bug in Rust**: Was advancing `ip` twice (once in match finding, once after)
   - **Fix**: Remove bound check on ref_ptr, and update ip correctly: `ip = ip + len - ipshift`

4. **Match Acceptance Criteria**:
   - Reject if `len < minlen` OR (`len <= 5` AND `distance >= MAX_DISTANCE`)
   - Accept if `len >= minlen` AND (`len > 5` OR `distance < MAX_DISTANCE`)

5. **Encoding**:
   - Length is biased by -2 for encoding
   - Short matches (len < 7): 2 bytes overhead
   - Long matches (len >= 7): 2+ bytes overhead (additional bytes for extended length)
   - Far matches (distance >= MAX_DISTANCE): +2 bytes overhead

### Bitshuffle Filter

The `bitshuffle` implementation in `blusc` seems to produce different output than `c-blosc2` for the `roundtrip` test case (sequential data).
- `blusc` produces a pattern that `blosclz` compresses to ~5000 bytes (2:1 ratio).
- `c-blosc2` produces a pattern that compresses to ~2000 bytes (40:1 ratio).
- This suggests `c-blosc2` bitshuffle output is more regular/compressible for sequential data.
- `blusc` bitshuffle passes roundtrip test (shuffle -> unshuffle), so it is reversible.
- The difference might be in `bshuf_trans_bitrow_eight` or `trans_bit_8x8` logic, or block size handling.

### Compression Ratio Mismatch

- `test_roundtrip_csv_case`: `blusc` (3064) is smaller than `bound` (4444). `blusc` is better.
- `test_roundtrip_large_bitshuffle`: `blusc` (8828) is larger than `bound` (2188). `blusc` is worse.
- This suggests `blusc` finds more matches in some cases (csv), but fails to find matches in others (bitshuffle) due to data differences.