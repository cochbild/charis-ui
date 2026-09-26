//! A fast, non-cryptographic hasher for the runtime's internal maps.
//!
//! The keys are element ids (already well-mixed hashes) and small integers;
//! SipHash's DoS resistance isn't needed, and its cost showed up in
//! per-element work (layout sync, id lookups).

use std::hash::BuildHasherDefault;

/// FxHash-style hasher (as used by rustc): a rotate, xor and multiply per
/// word, which spreads small integer keys across the whole hash.
#[derive(Default, Clone, Copy)]
pub(crate) struct FxHasher(u64);

const K: u64 = 0x51_7c_c1_b7_27_22_0a_95;

impl std::hash::Hasher for FxHasher {
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut b = [0u8; 8];
            b[..chunk.len()].copy_from_slice(chunk);
            self.write_u64(u64::from_le_bytes(b));
        }
    }
    #[inline]
    fn write_u64(&mut self, v: u64) {
        self.0 = (self.0.rotate_left(5) ^ v).wrapping_mul(K);
    }
    #[inline]
    fn write_u32(&mut self, v: u32) {
        self.write_u64(v as u64);
    }
    #[inline]
    fn write_u16(&mut self, v: u16) {
        self.write_u64(v as u64);
    }
    #[inline]
    fn write_u8(&mut self, v: u8) {
        self.write_u64(v as u64);
    }
    #[inline]
    fn write_usize(&mut self, v: usize) {
        self.write_u64(v as u64);
    }
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

pub(crate) type FxBuild = BuildHasherDefault<FxHasher>;
pub(crate) type FxHashMap<K, V> = std::collections::HashMap<K, V, FxBuild>;
pub(crate) type FxHashSet<K> = std::collections::HashSet<K, FxBuild>;
/// Map keyed by already-hashed u64s.
pub(crate) type IdMap<V> = FxHashMap<u64, V>;
