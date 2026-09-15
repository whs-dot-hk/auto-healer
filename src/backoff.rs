// Copyright (c) 수영 책방 Swimming Bookstore

use std::time::{Duration, Instant};

pub struct Backoff {
    base: Duration,
    cap: Duration,
    current: Duration,
    quiet_until: Option<Instant>,
}

impl Backoff {
    pub fn new(base: Duration, cap: Duration) -> Self {
        Self {
            base,
            cap: cap.max(base),
            current: base,
            quiet_until: None,
        }
    }

    pub fn remaining(&self, now: Instant) -> Option<Duration> {
        self.quiet_until
            .and_then(|until| until.checked_duration_since(now))
            .filter(|d| !d.is_zero())
    }

    pub fn on_fire(&mut self, now: Instant) {
        self.quiet_until = Some(now + self.current);
        self.current = self.current.saturating_mul(2).min(self.cap).max(self.current);
    }

    pub fn on_healthy(&mut self) -> bool {
        let reset = self.current != self.base || self.quiet_until.is_some();
        self.current = self.base;
        self.quiet_until = None;
        reset
    }

    pub fn current(&self) -> Duration {
        self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_fire_uses_base() {
        let mut b = Backoff::new(Duration::from_secs(5), Duration::from_secs(40));
        let t0 = Instant::now();
        b.on_fire(t0);
        assert_eq!(b.remaining(t0), Some(Duration::from_secs(5)));
        assert_eq!(b.remaining(t0 + Duration::from_secs(5)), None);
        assert_eq!(b.current(), Duration::from_secs(10));
    }

    #[test]
    fn doubles_until_cap() {
        let mut b = Backoff::new(Duration::from_secs(5), Duration::from_secs(20));
        let t0 = Instant::now();
        b.on_fire(t0);
        b.on_fire(t0);
        b.on_fire(t0);
        assert_eq!(b.current(), Duration::from_secs(20));
        b.on_fire(t0);
        assert_eq!(b.current(), Duration::from_secs(20));
        assert_eq!(b.remaining(t0), Some(Duration::from_secs(20)));
    }

    #[test]
    fn healthy_resets() {
        let mut b = Backoff::new(Duration::from_secs(5), Duration::from_secs(40));
        let t0 = Instant::now();
        b.on_fire(t0);
        assert!(b.on_healthy());
        assert_eq!(b.current(), Duration::from_secs(5));
        assert_eq!(b.remaining(t0), None);
        assert!(!b.on_healthy());
    }
}
