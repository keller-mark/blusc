use blusc::api::{
    blosc2_compress as blusc_blosc2_compress,
    blosc2_decompress as blusc_blosc2_decompress,
    blosc2_create_cctx as blusc_blosc2_create_cctx,
    blosc2_compress_ctx as blusc_blosc2_compress_ctx,
    blosc2_create_dctx as blusc_blosc2_create_dctx,
    blosc2_decompress_ctx as blusc_blosc2_decompress_ctx,
    BLOSC2_CPARAMS_DEFAULTS as BLUSC_BLOSC2_CPARAMS_DEFAULTS,
    BLOSC2_DPARAMS_DEFAULTS as BLUSC_BLOSC2_DPARAMS_DEFAULTS,
};

pub fn convert_from_bytes_slice<T: bytemuck::Pod>(from: &[u8]) -> Vec<T> {
    bytemuck::allocation::pod_collect_to_vec(from)
}

/// Transmute from `Vec<u8>` to `Vec<T>`.
pub fn transmute_from_bytes_vec<T: bytemuck::Pod>(from: Vec<u8>) -> Vec<T> {
    bytemuck::allocation::try_cast_vec(from)
        .unwrap_or_else(|(_err, from)| convert_from_bytes_slice(&from))
}

/// Convert from `&[T]` to `Vec<u8>`.
pub fn convert_to_bytes_vec<T: bytemuck::NoUninit>(from: &[T]) -> Vec<u8> {
    bytemuck::allocation::pod_collect_to_vec(from)
}

/// Transmute from `Vec<T>` to `Vec<u8>`.
pub fn transmute_to_bytes_vec<T: bytemuck::NoUninit>(from: Vec<T>) -> Vec<u8> {
    bytemuck::allocation::try_cast_vec(from)
        .unwrap_or_else(|(_err, from)| convert_to_bytes_vec(&from))
}

/// The decoded representation of `bytes`.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum BytesRepresentation {
    /// The output size is fixed.
    FixedSize(u64),
    /// The output size is bounded.
    BoundedSize(u64),
    /// The output size is unbounded/indeterminate.
    UnboundedSize,
}

impl BytesRepresentation {
    /// Return the fixed or bounded size of the bytes representations, or [`None`] if the size is unbounded.
    pub const fn size(&self) -> Option<u64> {
        match self {
            Self::FixedSize(size) | Self::BoundedSize(size) => Some(*size),
            Self::UnboundedSize => None,
        }
    }
}

const JSON_VALID1: &str = r#"
{
    "cname": "lz4",
    "clevel": 5,
    "shuffle": "shuffle",
    "typesize": 2,
    "blocksize": 0
}"#;


#[test]
fn codec_blosc_round_trip1() {
    // codec_blosc_round_trip1
    let ground_truth = [0, 0, 1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 8, 0, 9, 0, 10, 0, 11, 0, 12, 0, 13, 0, 14, 0, 15, 0, 16, 0, 17, 0, 18, 0, 19, 0, 20, 0, 21, 0, 22, 0, 23, 0, 24, 0, 25, 0, 26, 0, 27, 0, 28, 0, 29, 0, 30, 0, 31, 0];

    let elements: Vec<u16> = (0..32).collect();
    let bytes = transmute_to_bytes_vec(elements);
    let bytes_representation = BytesRepresentation::FixedSize(bytes.len() as u64);

    assert_eq!(bytes, ground_truth);

    // TODO: blosc_compress_ctx(clevel: 5, doshuffle: Shuffle, typesize: 2, nbytes: 64, destsize 0, compressor LZ4, bloscksize: 0)
    

    assert_eq!(decoded, ground_truth)
    
    
}