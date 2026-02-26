#![allow(dead_code)]

use std::time::{Duration, Instant};

const MIN_TEXT_LEN: usize = 4;
const MAX_TEXT_LEN: usize = 1500;

pub struct Debouncer {
    delay: Duration,
    last_emitted_hash: Option<u64>,
    pending_hash: Option<u64>,
    pending_text: Option<String>,
    pending_since: Option<Instant>,
}

impl Debouncer {
    #[must_use]
    pub const fn new(delay_ms: u64) -> Self {
        Self {
            delay: Duration::from_millis(delay_ms),
            last_emitted_hash: None,
            pending_hash: None,
            pending_text: None,
            pending_since: None,
        }
    }

    pub fn feed(&mut self, text: &str) -> Option<String> {
        let filtered = filter_text(text)?;
        let hash = fnv1a64(filtered.as_bytes());

        if self.last_emitted_hash == Some(hash) {
            return None;
        }

        match self.pending_hash {
            Some(existing) if existing == hash => {
                if self.pending_since.is_some_and(|since| since.elapsed() >= self.delay) {
                    self.last_emitted_hash = Some(hash);
                    self.pending_hash = None;
                    self.pending_since = None;
                    return self.pending_text.take();
                }
                None
            }
            _ => {
                self.pending_hash = Some(hash);
                self.pending_since = Some(Instant::now());
                self.pending_text = Some(filtered.to_string());
                None
            }
        }
    }
}

#[must_use]
pub fn filter_text(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    if (MIN_TEXT_LEN..=MAX_TEXT_LEN).contains(&trimmed.len()) { Some(trimmed) } else { None }
}

fn fnv1a64(data: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0100_0000_01b3;

    let mut hash = OFFSET;
    for byte in data {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}
