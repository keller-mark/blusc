// Corresponds to c-blosc2/blosc/blosc-private.h

/*********************************************************************

  Utility functions meant to be used internally.

*********************************************************************/

// Return true if platform is little endian; else false
fn is_little_endian() -> bool {
    // static const int i = 1;
    // char* p = (char*)&i;
    let i: i32 = 1;
    let p = i.to_ne_bytes();
    
    // if (p[0] == 1) {
    //     return true;
    // }
    // else {
    //     return false;
    // }
    p[0] == 1
}

fn endian_handler(little: bool, dest: &mut [u8], pa: &[u8], size: usize) {
    // bool little_endian = is_little_endian();
    let little_endian = is_little_endian();
    
    // if (little_endian == little) {
    //     memcpy(dest, pa, size);
    // }
    if little_endian == little {
        dest[..size].copy_from_slice(&pa[..size]);
    } else {
        // uint8_t* pa_ = (uint8_t*)pa;
        // uint8_t pa2_[8];
        let mut pa2 = [0u8; 8];
        
        // switch (size) {
        match size {
            // case 8:
            //     pa2_[0] = pa_[7];
            //     pa2_[1] = pa_[6];
            //     pa2_[2] = pa_[5];
            //     pa2_[3] = pa_[4];
            //     pa2_[4] = pa_[3];
            //     pa2_[5] = pa_[2];
            //     pa2_[6] = pa_[1];
            //     pa2_[7] = pa_[0];
            //     break;
            8 => {
                pa2[0] = pa[7];
                pa2[1] = pa[6];
                pa2[2] = pa[5];
                pa2[3] = pa[4];
                pa2[4] = pa[3];
                pa2[5] = pa[2];
                pa2[6] = pa[1];
                pa2[7] = pa[0];
            }
            // case 4:
            //     pa2_[0] = pa_[3];
            //     pa2_[1] = pa_[2];
            //     pa2_[2] = pa_[1];
            //     pa2_[3] = pa_[0];
            //     break;
            4 => {
                pa2[0] = pa[3];
                pa2[1] = pa[2];
                pa2[2] = pa[1];
                pa2[3] = pa[0];
            }
            // case 2:
            //     pa2_[0] = pa_[1];
            //     pa2_[1] = pa_[0];
            //     break;
            2 => {
                pa2[0] = pa[1];
                pa2[1] = pa[0];
            }
            // case 1:
            //     pa2_[0] = pa_[0];
            //     break;
            1 => {
                pa2[0] = pa[0];
            }
            // default:
            //     BLOSC_TRACE_ERROR("Unhandled size: %d.", size);
            _ => {
                eprintln!("Unhandled size: {}", size);
            }
        }
        // memcpy(dest, pa2_, size);
        dest[..size].copy_from_slice(&pa2[..size]);
    }
}

pub fn to_little(dest: &mut [u8], src: &[u8], itemsize: usize) {
    // #define to_little(dest, src, itemsize)    endian_handler(true, dest, src, itemsize)
    endian_handler(true, dest, src, itemsize);
}

pub fn from_little(dest: &mut [u8], src: &[u8], itemsize: usize) {
    // #define from_little(dest, src, itemsize)  endian_handler(true, dest, src, itemsize)
    endian_handler(true, dest, src, itemsize);
}

pub fn to_big(dest: &mut [u8], src: &[u8], itemsize: usize) {
    // #define to_big(dest, src, itemsize)       endian_handler(false, dest, src, itemsize)
    endian_handler(false, dest, src, itemsize);
}

pub fn from_big(dest: &mut [u8], src: &[u8], itemsize: usize) {
    // #define from_big(dest, src, itemsize)     endian_handler(false, dest, src, itemsize)
    endian_handler(false, dest, src, itemsize);
}

/* Copy 4 bytes from @p *pa to int32_t, changing endianness if necessary. */
fn sw32_(pa: &[u8]) -> i32 {
    // int32_t idest;
    // bool little_endian = is_little_endian();
    let little_endian = is_little_endian();
    
    // if (little_endian) {
    //     memcpy(&idest, pa, sizeof(idest));
    // }
    if little_endian {
        i32::from_le_bytes([pa[0], pa[1], pa[2], pa[3]])
    } else {
        // #if defined (__GNUC__)
        //     return __builtin_bswap32(*(unsigned int *)pa);
        // #elif defined (_MSC_VER) /* Visual Studio */
        //     return _byteswap_ulong(*(unsigned int *)pa);
        // #else
        //     uint8_t *dest = (uint8_t *)&idest;
        //     dest[0] = pa_[3];
        //     dest[1] = pa_[2];
        //     dest[2] = pa_[1];
        //     dest[3] = pa_[0];
        // #endif
        i32::from_be_bytes([pa[0], pa[1], pa[2], pa[3]]).swap_bytes()
    }
}

/* Copy 4 bytes from int32_t to @p *dest, changing endianness if necessary. */
fn _sw32(dest: &mut [u8], a: i32) {
    // uint8_t* dest_ = (uint8_t*)dest;
    // uint8_t* pa = (uint8_t*)&a;
    // bool little_endian = is_little_endian();
    let little_endian = is_little_endian();
    
    // if (little_endian) {
    //     memcpy(dest_, &a, sizeof(a));;
    // }
    if little_endian {
        let bytes = a.to_le_bytes();
        dest[..4].copy_from_slice(&bytes);
    } else {
        // #if defined (__GNUC__)
        //     *(int32_t *)dest_ = __builtin_bswap32(*(unsigned int *)pa);
        // #elif defined (_MSC_VER) /* Visual Studio */
        //     *(int32_t *)dest_ = _byteswap_ulong(*(unsigned int *)pa);
        // #else
        //     dest_[0] = pa[3];
        //     dest_[1] = pa[2];
        //     dest_[2] = pa[1];
        //     dest_[3] = pa[0];
        // #endif
        let bytes = a.to_be_bytes();
        dest[0] = bytes[3];
        dest[1] = bytes[2];
        dest[2] = bytes[1];
        dest[3] = bytes[0];
    }
}

/* Reverse swap bits in a 32-bit integer */
fn bswap32_(a: i32) -> i32 {
    // #if defined (__GNUC__)
    //   return __builtin_bswap32(a);
    // 
    // #elif defined (_MSC_VER) /* Visual Studio */
    //   return _byteswap_ulong(a);
    // #else
    //   a = ((a & 0x000000FF) << 24) |
    //       ((a & 0x0000FF00) <<  8) |
    //       ((a & 0x00FF0000) >>  8) |
    //       ((a & 0xFF000000) >> 24);
    //   return a;
    // #endif
    a.swap_bytes()
}

/*
Original C code from c-blosc2/blosc/blosc-private.h:

#include "blosc2/blosc2-common.h"
#include "blosc2.h"

/***********************************************************************

  Utility functions meant to be used internally.

***********************************************************************/

#define to_little(dest, src, itemsize)    endian_handler(true, dest, src, itemsize)
#define from_little(dest, src, itemsize)  endian_handler(true, dest, src, itemsize)
#define to_big(dest, src, itemsize)       endian_handler(false, dest, src, itemsize)
#define from_big(dest, src, itemsize)     endian_handler(false, dest, src, itemsize)


// Return true if platform is little endian; else false
static bool is_little_endian(void) {
  static const int i = 1;
  char* p = (char*)&i;

  if (p[0] == 1) {
    return true;
  }
  else {
    return false;
  }
}


static inline void endian_handler(bool little, void *dest, const void *pa, int size)
{
  bool little_endian = is_little_endian();
  if (little_endian == little) {
    memcpy(dest, pa, size);
  }
  else {
    uint8_t* pa_ = (uint8_t*)pa;
    uint8_t pa2_[8];
    switch (size) {
      case 8:
        pa2_[0] = pa_[7];
        pa2_[1] = pa_[6];
        pa2_[2] = pa_[5];
        pa2_[3] = pa_[4];
        pa2_[4] = pa_[3];
        pa2_[5] = pa_[2];
        pa2_[6] = pa_[1];
        pa2_[7] = pa_[0];
        break;
      case 4:
        pa2_[0] = pa_[3];
        pa2_[1] = pa_[2];
        pa2_[2] = pa_[1];
        pa2_[3] = pa_[0];
        break;
      case 2:
        pa2_[0] = pa_[1];
        pa2_[1] = pa_[0];
        break;
      case 1:
        pa2_[0] = pa_[0];
        break;
      default:
        BLOSC_TRACE_ERROR("Unhandled size: %d.", size);
    }
    memcpy(dest, pa2_, size);
  }
}

/* Copy 4 bytes from @p *pa to int32_t, changing endianness if necessary. */
static inline int32_t sw32_(const void* pa) {
  int32_t idest;

  bool little_endian = is_little_endian();
  if (little_endian) {
    memcpy(&idest, pa, sizeof(idest));
  }
  else {
#if defined (__GNUC__)
    return __builtin_bswap32(*(unsigned int *)pa);
#elif defined (_MSC_VER) /* Visual Studio */
    return _byteswap_ulong(*(unsigned int *)pa);
#else
    uint8_t *dest = (uint8_t *)&idest;
    dest[0] = pa_[3];
    dest[1] = pa_[2];
    dest[2] = pa_[1];
    dest[3] = pa_[0];
#endif
  }
  return idest;
}

/* Copy 4 bytes from int32_t to @p *dest, changing endianness if necessary. */
static inline void _sw32(void* dest, int32_t a) {
  uint8_t* dest_ = (uint8_t*)dest;
  uint8_t* pa = (uint8_t*)&a;

  bool little_endian = is_little_endian();
  if (little_endian) {
    memcpy(dest_, &a, sizeof(a));;
  }
  else {
#if defined (__GNUC__)
    *(int32_t *)dest_ = __builtin_bswap32(*(unsigned int *)pa);
#elif defined (_MSC_VER) /* Visual Studio */
    *(int32_t *)dest_ = _byteswap_ulong(*(unsigned int *)pa);
#else
    dest_[0] = pa[3];
    dest_[1] = pa[2];
    dest_[2] = pa[1];
    dest_[3] = pa[0];
#endif
  }
}

/* Reverse swap bits in a 32-bit integer */
static inline int32_t bswap32_(int32_t a) {
#if defined (__GNUC__)
  return __builtin_bswap32(a);

#elif defined (_MSC_VER) /* Visual Studio */
  return _byteswap_ulong(a);
#else
  a = ((a & 0x000000FF) << 24) |
      ((a & 0x0000FF00) <<  8) |
      ((a & 0x00FF0000) >>  8) |
      ((a & 0xFF000000) >> 24);
  return a;
#endif
}


*/