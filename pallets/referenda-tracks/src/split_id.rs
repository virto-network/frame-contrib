use codec::MaxEncodedLen;
use frame_support::{traits::Incrementable, Parameter};
use sp_runtime::traits::Member;

pub trait SplitId: Sized {
    type Half: Default + Incrementable + Member + Parameter + MaxEncodedLen;

    fn split(self) -> (Self::Half, Self::Half);
    fn combine(group: Self::Half, track: Self::Half) -> Self;
}

macro_rules! impl_split_id {
    ($type:ty, $half:ty, $bits:expr) => {
        impl SplitId for $type {
            type Half = $half;

            fn split(self) -> (Self::Half, Self::Half) {
                let half_bits = $bits / 2;
                let mask = (1 << half_bits) - 1;
                let group = self >> half_bits;
                let track = self & mask;
                (group as $half, track as $half)
            }

            fn combine(group: Self::Half, track: Self::Half) -> Self {
                let group = group as $type;
                let track = track as $type;
                let half_bits = $bits / 2;
                let mask = (1 << half_bits) - 1;
                (group << half_bits) | (track & mask)
            }
        }
    };
}

impl_split_id!(u16, u8, 16);
impl_split_id!(u32, u16, 32);
impl_split_id!(u64, u32, 64);
impl_split_id!(u128, u64, 128);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u16_split() {
        let id: u16 = 257;
        let (group, track) = id.split();
        assert_eq!(group, 1);
        assert_eq!(track, 1);
    }

    #[test]
    fn test_u32_split() {
        let id: u32 = 65537;
        let (group, track) = id.split();
        assert_eq!(group, 1);
        assert_eq!(track, 1);
    }

    #[test]
    fn test_u64_split() {
        let id: u64 = 4294967297;
        let (group, track) = id.split();
        assert_eq!(group, 1);
        assert_eq!(track, 1);
    }

    #[test]
    fn test_zero_split() {
        let id: u16 = 0;
        let (group, track) = id.split();
        assert_eq!(group, 0);
        assert_eq!(track, 0);
    }

    #[test]
    fn test_max_values() {
        let id = u128::MAX;
        let (group, track) = id.split();
        assert_eq!(group, u64::MAX);
        assert_eq!(track, u64::MAX);
    }

    #[test]
    fn test_combine_u16() {
        let combined = u16::combine(1, 1);
        assert_eq!(combined, 257);
    }

    #[test]
    fn test_combine_u32() {
        let combined = u32::combine(1, 1);
        assert_eq!(combined, 65537);
    }

    #[test]
    fn test_combine_u64() {
        let combined = u64::combine(1, 1);
        assert_eq!(combined, 4294967297);
    }

    #[test]
    fn test_split_combine_roundtrip() {
        let original: u32 = 0x12345678;
        let (group, track) = original.split();
        let reconstructed = u32::combine(group, track);
        assert_eq!(original, reconstructed);
    }
}
