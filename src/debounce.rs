use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub struct Debouncer {
    delay: Duration,
    last_hash: AtomicU64,
    last_emit: Option<Instant>,
}

impl Debouncer {
    pub fn new(delay_ms: u64) -> Self {
        Self {
            delay: Duration::from_millis(delay_ms),
            last_hash: AtomicU64::new(0),
            last_emit: None,
        }
    }

    pub fn feed(&mut self, text: &str) -> Option<String> {
        let hash = fnv1a64(text.as_bytes());
        if self.last_hash.load(Ordering::Relaxed) == hash {
            return None;
        }

        if let Some(last_emit) = self.last_emit {
            if last_emit.elapsed() < self.delay {
                return None;
            }
        }

        self.last_hash.store(hash, Ordering::Relaxed);
        self.last_emit = Some(Instant::now());
        Some(text.to_string())
    }
}

fn fnv1a64(data: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    let mut hash = OFFSET;
    for byte in data {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}
