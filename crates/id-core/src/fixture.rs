//! Stable pseudo-random input derivation for reproducible test fixtures.
//!
//! This is deliberately not a cryptographic RNG. Its byte-for-byte behaviour is
//! part of `GENERATOR_VERSION`; callers that need unpredictable values must omit
//! `fixture_seed` and let the API create an ephemeral seed first.

/// Version of the deterministic fixture algorithms exposed by the API.
pub const GENERATOR_VERSION: &str = "1";

#[derive(Debug, Clone)]
pub(crate) struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    pub(crate) fn new(seed: &str, namespace: &str, index: u32) -> Self {
        // FNV-1a gives us a platform-independent initial state. SplitMix64 below
        // then provides a well-distributed deterministic stream.
        let mut state = 0xcbf2_9ce4_8422_2325_u64;
        for byte in b"nrg-generator-v1\0"
            .iter()
            .chain(seed.as_bytes())
            .chain([0].iter())
            .chain(namespace.as_bytes())
            .chain([0].iter())
            .chain(index.to_le_bytes().iter())
        {
            state ^= u64::from(*byte);
            state = state.wrapping_mul(0x0000_0100_0000_01b3);
        }

        Self { state }
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    pub(crate) fn index(&mut self, upper_bound: usize) -> usize {
        debug_assert!(upper_bound > 0);
        (self.next_u64() % upper_bound as u64) as usize
    }

    pub(crate) fn digit(&mut self) -> u8 {
        self.index(10) as u8
    }

    pub(crate) fn nonzero_digit(&mut self) -> u8 {
        self.index(9) as u8 + 1
    }

    pub(crate) fn uppercase_alphanumeric(&mut self) -> char {
        const CHARACTERS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        CHARACTERS[self.index(CHARACTERS.len())] as char
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_is_reproducible_and_namespaced() {
        let mut first = DeterministicRng::new("fixture-4711", "iban", 0);
        let mut second = DeterministicRng::new("fixture-4711", "iban", 0);
        let first_values = [first.next_u64(), first.next_u64(), first.next_u64()];
        let second_values = [second.next_u64(), second.next_u64(), second.next_u64()];

        assert_eq!(first_values, second_values);

        let mut other_kind = DeterministicRng::new("fixture-4711", "bic", 0);
        let mut other_index = DeterministicRng::new("fixture-4711", "iban", 1);
        assert_ne!(first_values[0], other_kind.next_u64());
        assert_ne!(first_values[0], other_index.next_u64());
    }
}
