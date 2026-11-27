//! Button debouncing and rotary encoder decoding.
//!
//! Both are pure state machines: the firmware only feeds them GPIO samples and
//! timestamps, which keeps the timing-sensitive part testable on the host.

/// A debounced button edge, including auto-repeat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// The button became pressed.
    Pressed,
    /// The button became released.
    Released,
    /// The button has been held past the long-press threshold.
    LongPress,
    /// Auto-repeat while held.
    Repeat,
}

/// Timing thresholds for [`Debouncer`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DebounceConfig {
    /// Minimum stable time before a raw change is accepted, in microseconds.
    pub debounce_us: u32,
    /// Time held before a [`Edge::LongPress`] is emitted.
    pub long_press_us: u32,
    /// Delay between the long press and the first repeat.
    pub repeat_delay_us: u32,
    /// Interval between repeats while held.
    pub repeat_us: u32,
}

impl Default for DebounceConfig {
    fn default() -> Self {
        Self {
            debounce_us: 15_000,
            long_press_us: 500_000,
            repeat_delay_us: 200_000,
            repeat_us: 100_000,
        }
    }
}

impl DebounceConfig {
    /// Creates a configuration.
    pub const fn new(
        debounce_us: u32,
        long_press_us: u32,
        repeat_delay_us: u32,
        repeat_us: u32,
    ) -> Self {
        Self {
            debounce_us,
            long_press_us,
            repeat_delay_us,
            repeat_us,
        }
    }
}

/// Debounces one active-low button and generates long press / repeat events.
#[derive(Clone, Copy, Debug)]
pub struct Debouncer {
    cfg: DebounceConfig,
    stable: bool,
    candidate: bool,
    candidate_since: u64,
    pressed_since: u64,
    long_fired: bool,
    next_repeat: u64,
}

impl Debouncer {
    /// Creates a debouncer for a released button.
    pub const fn new(cfg: DebounceConfig) -> Self {
        Self {
            cfg,
            stable: false,
            candidate: false,
            candidate_since: 0,
            pressed_since: 0,
            long_fired: false,
            next_repeat: 0,
        }
    }

    /// Whether the debounced button is currently pressed.
    pub const fn is_pressed(&self) -> bool {
        self.stable
    }

    /// The active configuration.
    pub const fn config(&self) -> DebounceConfig {
        self.cfg
    }

    /// Feeds one raw sample. `raw_pressed` is `true` when the button is down.
    pub fn update(&mut self, raw_pressed: bool, now_us: u64) -> Option<Edge> {
        if raw_pressed != self.candidate {
            self.candidate = raw_pressed;
            self.candidate_since = now_us;
        }

        if self.candidate != self.stable
            && now_us.saturating_sub(self.candidate_since) >= self.cfg.debounce_us as u64
        {
            self.stable = self.candidate;
            if self.stable {
                self.pressed_since = now_us;
                self.long_fired = false;
                return Some(Edge::Pressed);
            }
            self.long_fired = false;
            return Some(Edge::Released);
        }

        // Only emit long press / repeat when the raw level agrees with the
        // debounced level; a release that is still inside the debounce window
        // must not produce repeats.
        if self.stable && self.candidate == self.stable {
            if !self.long_fired
                && now_us.saturating_sub(self.pressed_since) >= self.cfg.long_press_us as u64
            {
                self.long_fired = true;
                self.next_repeat = now_us.saturating_add(self.cfg.repeat_delay_us as u64);
                return Some(Edge::LongPress);
            }
            if self.long_fired && self.cfg.repeat_us > 0 && now_us >= self.next_repeat {
                self.next_repeat = now_us.saturating_add(self.cfg.repeat_us as u64);
                return Some(Edge::Repeat);
            }
        }
        None
    }
}

/// Decodes a quadrature encoder into detent steps.
#[derive(Clone, Copy, Debug)]
pub struct EncoderDecoder {
    last_state: u8,
    quarter_steps: i16,
}

// Indexed by `previous_state * 4 + current_state`, where a state is
// `(a as u8) | ((b as u8) << 1)`. Clockwise is 0 -> 1 -> 3 -> 2 -> 0.
const QUADRATURE: [i8; 16] = [
    0, 1, -1, 0, // previous 0
    -1, 0, 0, 1, // previous 1
    1, 0, 0, -1, // previous 2
    0, -1, 1, 0, // previous 3
];

impl Default for EncoderDecoder {
    fn default() -> Self {
        Self::new(false, false)
    }
}

impl EncoderDecoder {
    /// Creates a decoder from the initial pin levels.
    pub const fn new(a: bool, b: bool) -> Self {
        Self {
            last_state: (a as u8) | ((b as u8) << 1),
            quarter_steps: 0,
        }
    }

    /// Feeds one sample pair, returning emitted detent steps (`-1`, `0` or `1`).
    pub fn update(&mut self, a: bool, b: bool) -> i16 {
        let state = (a as u8) | ((b as u8) << 1);
        let delta = QUADRATURE[(self.last_state << 2 | state) as usize] as i16;
        self.last_state = state;
        self.quarter_steps += delta;
        let steps = self.quarter_steps / 4;
        self.quarter_steps -= steps * 4;
        steps
    }

    /// Discards accumulated sub-detent movement (e.g. after a long pause).
    pub fn reset(&mut self) {
        self.quarter_steps = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> DebounceConfig {
        DebounceConfig::new(10, 100, 50, 20)
    }

    #[test]
    fn press_requires_stable_debounce_time() {
        let mut d = Debouncer::new(cfg());
        assert_eq!(d.update(true, 0), None);
        assert_eq!(d.update(false, 5), None); // bounce back before debounce
        assert_eq!(d.update(true, 6), None);
        assert_eq!(d.update(true, 15), None);
        assert_eq!(d.update(true, 16), Some(Edge::Pressed));
        assert!(d.is_pressed());
    }

    #[test]
    fn long_press_then_repeat_then_release() {
        let mut d = Debouncer::new(cfg());
        assert_eq!(d.update(true, 0), None);
        assert_eq!(d.update(true, 10), Some(Edge::Pressed));
        assert_eq!(d.update(true, 109), None);
        assert_eq!(d.update(true, 110), Some(Edge::LongPress));
        // repeat_delay 50 after the long press, then every 20.
        assert_eq!(d.update(true, 159), None);
        assert_eq!(d.update(true, 160), Some(Edge::Repeat));
        assert_eq!(d.update(true, 179), None);
        assert_eq!(d.update(true, 180), Some(Edge::Repeat));
        assert_eq!(d.update(false, 200), None);
        assert_eq!(d.update(false, 210), Some(Edge::Released));
        assert!(!d.is_pressed());
    }

    #[test]
    fn short_press_has_no_long_press() {
        let mut d = Debouncer::new(cfg());
        d.update(true, 0);
        assert_eq!(d.update(true, 10), Some(Edge::Pressed));
        assert_eq!(d.update(false, 50), None);
        assert_eq!(d.update(false, 60), Some(Edge::Released));
        assert_eq!(d.update(false, 500), None);
    }

    #[test]
    fn time_going_backwards_does_not_panic() {
        let mut d = Debouncer::new(cfg());
        d.update(true, 1000);
        assert_eq!(d.update(true, 5), None);
    }

    #[test]
    fn encoder_clockwise_one_detent() {
        let mut e = EncoderDecoder::new(false, false);
        let mut total = 0;
        // 0 -> 1 -> 3 -> 2 -> 0 is one full quadrature cycle = one detent.
        for (a, b) in [(true, false), (true, true), (false, true), (false, false)] {
            total += e.update(a, b);
        }
        assert_eq!(total, 1);
    }

    #[test]
    fn encoder_counter_clockwise() {
        let mut e = EncoderDecoder::new(false, false);
        let mut total = 0;
        for (a, b) in [(false, true), (true, true), (true, false), (false, false)] {
            total += e.update(a, b);
        }
        assert_eq!(total, -1);
    }

    #[test]
    fn encoder_bounce_cancels_out() {
        let mut e = EncoderDecoder::new(false, false);
        let mut total = 0;
        for sample in [
            (true, false),
            (false, false),
            (true, false),
            (true, true),
            (false, true),
            (false, false),
        ] {
            total += e.update(sample.0, sample.1);
        }
        assert_eq!(total, 1);
    }

    #[test]
    fn encoder_repeated_cycles_accumulate() {
        let mut e = EncoderDecoder::new(false, false);
        let mut total = 0;
        for _ in 0..3 {
            for (a, b) in [(true, false), (true, true), (false, true), (false, false)] {
                total += e.update(a, b);
            }
        }
        assert_eq!(total, 3);
    }
}
