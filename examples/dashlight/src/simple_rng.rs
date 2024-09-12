// simple_rng.rs

use rand_core::{RngCore, SeedableRng};

pub struct SimpleRng {
    state: u32,
}

impl SimpleRng {
    pub fn new(seed: [u8; 4]) -> Self {
        SimpleRng::from_seed(seed)
    }

    pub fn gen_bool(&mut self, probability: f64) -> bool {
        let random_value = self.next_u32();
        let max_value = u32::MAX;
        (random_value as f64 / max_value as f64) < probability
    }
}

impl SeedableRng for SimpleRng {
    type Seed = [u8; 4];

    fn from_seed(seed: Self::Seed) -> Self {
        let mut state = 0u32;
        for (i, &byte) in seed.iter().enumerate() {
            state |= (byte as u32) << (i * 8);
        }
        SimpleRng { state }
    }
}

impl RngCore for SimpleRng {
    fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    fn next_u64(&mut self) -> u64 {
        let high = self.next_u32();
        let low = self.next_u32();
        ((high as u64) << 32) | (low as u64)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for chunk in dest.chunks_mut(4) {
            let value = self.next_u32().to_le_bytes();
            chunk.copy_from_slice(&value[..chunk.len()]);
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

//impl Rng for SimpleRng {}
