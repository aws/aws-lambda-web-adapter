use std::time::Instant;

pub(crate) struct Checkpoint {
    start: Instant,
    interval_ms: u128,
    next_ms: u128,
}

impl Checkpoint {
    pub fn new() -> Checkpoint {
        // The default function timeout is 3 seconds. This will alert the users. See #520
        let interval_ms = 2000;

        let start = Instant::now();
        Checkpoint {
            start,
            interval_ms,
            next_ms: start.elapsed().as_millis() + interval_ms,
        }
    }

    pub const fn next_ms(&self) -> u128 {
        self.next_ms
    }

    pub const fn increment(&mut self) {
        self.next_ms += self.interval_ms;
    }

    pub fn lapsed(&self) -> bool {
        self.start.elapsed().as_millis() >= self.next_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_new() {
        let checkpoint = Checkpoint::new();
        assert_eq!(checkpoint.next_ms(), 2000);
        assert!(!checkpoint.lapsed());
    }

    #[test]
    fn test_checkpoint_increment() {
        let mut checkpoint = Checkpoint::new();
        checkpoint.increment();
        assert_eq!(checkpoint.next_ms(), 4000);
        assert!(!checkpoint.lapsed());
    }

    #[test]
    fn test_checkpoint_lapsed() {
        let checkpoint = Checkpoint {
            start: Instant::now(),
            interval_ms: 0,
            next_ms: 0,
        };
        assert!(checkpoint.lapsed());
    }
}
