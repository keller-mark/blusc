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

/*

#include "blosc2.h"

/* System-specific high-precision timing functions. */
#if defined(_WIN32)

#include <windows.h>

/* Set a timestamp value to the current time. */
void blosc_set_timestamp(blosc_timestamp_t* timestamp) {
  /* Ignore the return value, assume the call always succeeds. */
  QueryPerformanceCounter(timestamp);
}

/* Given two timestamp values, return the difference in nanoseconds. */
double blosc_elapsed_nsecs(blosc_timestamp_t start_time,
                           blosc_timestamp_t end_time) {
  LARGE_INTEGER CounterFreq;
  QueryPerformanceFrequency(&CounterFreq);

  return (double)(end_time.QuadPart - start_time.QuadPart) /
    ((double)CounterFreq.QuadPart / 1e9);
}

#else

#include <time.h>

#if defined(__MACH__) && defined(__APPLE__) // OS X does not have clock_gettime, use clock_get_time

#include <mach/clock.h>

/* Set a timestamp value to the current time. */
void blosc_set_timestamp(blosc_timestamp_t* timestamp) {
  clock_serv_t cclock;
  mach_timespec_t mts;
  host_get_clock_service(mach_host_self(), CALENDAR_CLOCK, &cclock);
  clock_get_time(cclock, &mts);
  mach_port_deallocate(mach_task_self(), cclock);
  timestamp->tv_sec = mts.tv_sec;
  timestamp->tv_nsec = mts.tv_nsec;
}

#else

/* Set a timestamp value to the current time. */
void blosc_set_timestamp(blosc_timestamp_t* timestamp) {
  clock_gettime(CLOCK_MONOTONIC, timestamp);
}

#endif

/* Given two timestamp values, return the difference in nanoseconds. */
double blosc_elapsed_nsecs(blosc_timestamp_t start_time,
                           blosc_timestamp_t end_time) {
  return (1e9 * (double)(end_time.tv_sec - start_time.tv_sec)) +
          (double)(end_time.tv_nsec - start_time.tv_nsec);
}

#endif

/* Given two timeval stamps, return the difference in seconds */
double blosc_elapsed_secs(blosc_timestamp_t last, blosc_timestamp_t current) {
  return 1e-9 * blosc_elapsed_nsecs(last, current);
}

 */
