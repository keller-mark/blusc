When porting logic to rust, use the C implementations in the `c-blosc` and `c-blosc2` subdirectories.

## Porting from C to Rust

When porting the implementation, try to reuse variable names and function names when possible, and use rust idioms otherwise.

The constants defined in `src/internal/constants.rs` are correct and verified against the reference implementation. Use these constants in your code.

We want to use the ported implementation in a single-threaded WebAssembly context, so IGNORE MULTI-THREADING and IGNORE THE FILESYSTEM.
DO EVERYTHING SINGLE-THREADED AND DO EVERYTHING IN-MEMORY.

WE DO NOT CARE ABOUT PERFORMANCE. IGNORE PROCESSOR-SPECIFIC OPTIMIZATIONS and IGNORE ARCHITECTURE-SPECIFIC HARDWARE OPTIMIZATIONS.

Leveraging libc is NOT ACCEPTABLE. Use pure rust. Do not use unsafe blocks.

## Note taking

Leave comments in the Rust code reflecting the logic used in the C implementation for future reference.
Preserve all comments from the original implementation.


## Unit testing

Be sure to run unit tests in single-threaded mode.

```sh
cargo test -- --test-threads=1
```

ALWAYS RUN ALL TESTS TO PREVENT REGRESSIONS. DO NOT FILTER TO INDIVIDUAL TESTS.


## Debugging

Debug by printing and comparing the return values from blosc2_src (correct) and blusc (incorrect, needs fixing) crates.
