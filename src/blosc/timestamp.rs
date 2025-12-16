// Corresponds to c-blosc2/blosc/timestamp.c

use std::time::Instant;

/// The type of timestamp used in this system.
/// In the C implementation, this is either LARGE_INTEGER (Windows) or struct timespec (Unix).
/// For our pure Rust implementation, we use std::time::Instant which is cross-platform.
pub type BloscTimestamp = Instant;

/// Set a timestamp value to the current time.
/// 
/// Corresponds to blosc_set_timestamp in C.
/// On Windows, the C version uses QueryPerformanceCounter.
/// On macOS, it uses clock_get_time with CALENDAR_CLOCK.
/// On other Unix systems, it uses clock_gettime with CLOCK_MONOTONIC.
pub fn blosc_set_timestamp() -> BloscTimestamp {
    Instant::now()
}

/// Given two timestamp values, return the difference in nanoseconds.
/// 
/// Corresponds to blosc_elapsed_nsecs in C.
/// On Windows, the C version calculates: (end - start) / (freq / 1e9)
/// On Unix, it calculates: 1e9 * (end.tv_sec - start.tv_sec) + (end.tv_nsec - start.tv_nsec)
pub fn blosc_elapsed_nsecs(start_time: BloscTimestamp, end_time: BloscTimestamp) -> f64 {
    let duration = end_time.duration_since(start_time);
    duration.as_nanos() as f64
}

/// Given two timestamp values, return the difference in seconds.
/// 
/// Corresponds to blosc_elapsed_secs in C.
pub fn blosc_elapsed_secs(last: BloscTimestamp, current: BloscTimestamp) -> f64 {
    blosc_elapsed_nsecs(last, current) * 1e-9
}
