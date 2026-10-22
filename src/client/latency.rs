//! Simulated network conditions between each client and the server.
//!
//! Each connection draws a base round trip. One-way delays add uniform jitter
//! and congestion episodes; replies incur processing time. Delays use continuous
//! seconds; arrivals apply on physics ticks. Clients only measure pings and
//! their own reply time.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct LatencyConfig {
    /// Base round trips a connection starts from, one picked per connection by
    /// `round_trip_weights` (empty: equally). None configured means zero.
    pub round_trip_seconds: Vec<f64>,
    pub round_trip_weights: Vec<f64>,
    /// Standard deviation of the log base round trip around the picked value.
    pub round_trip_spread: f64,
    /// Share of every round trip spent on the uplink.
    pub uplink_share: f64,
    /// Uniform spread of each one-way delay around its base.
    pub jitter_seconds: f64,
    pub congestion: Congestion,
    /// Base time from a decision to sending its command, drawn uniformly from this
    /// range per connection. Pings, sent on the client's own clock, do not wait.
    pub processing_seconds: [f64; 2],
    /// Uniform spread of each reply's processing time around the connection's base.
    pub processing_jitter_seconds: f64,
}
impl Default for LatencyConfig {
    fn default() -> Self {
        Self {
            round_trip_seconds: Vec::new(),
            round_trip_weights: Vec::new(),
            round_trip_spread: 0.,
            uplink_share: 0.5,
            jitter_seconds: 0.,
            congestion: Congestion::default(),
            processing_seconds: [0.; 2],
            processing_jitter_seconds: 0.,
        }
    }
}
impl LatencyConfig {
    /// A fixed round trip split evenly, without jitter or congestion.
    pub fn fixed(round_trip_seconds: f64) -> Self {
        Self {
            round_trip_seconds: vec![round_trip_seconds],
            ..Default::default()
        }
    }
    /// Fixed one-way delays, without jitter or congestion.
    pub fn one_way(uplink_seconds: f64, downlink_seconds: f64) -> Self {
        let round_trip = uplink_seconds + downlink_seconds;
        Self {
            uplink_share: if round_trip > 0. {
                uplink_seconds / round_trip
            } else {
                0.5
            },
            ..Self::fixed(round_trip)
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self
            .round_trip_seconds
            .iter()
            .chain([&self.jitter_seconds, &self.processing_jitter_seconds])
            .any(|v| !v.is_finite() || !(0. ..=2.).contains(v))
        {
            return Err("Latency must be 0 to 2 seconds".into());
        }
        let weights = &self.round_trip_weights;
        let total: f64 = weights.iter().sum();
        if !weights.is_empty()
            && (weights.len() != self.round_trip_seconds.len()
                || weights.iter().any(|w| !w.is_finite() || *w < 0.)
                || !total.is_finite()
                || total <= 0.)
        {
            return Err("Round-trip weights need one non-negative weight per round trip".into());
        }
        if !self.round_trip_spread.is_finite() || !(0. ..=2.).contains(&self.round_trip_spread) {
            return Err("Round-trip spread must be from 0 to 2".into());
        }
        if !(0. ..=1.).contains(&self.uplink_share) {
            return Err("Uplink share must be from 0 to 1".into());
        }
        let [least, most] = self.processing_seconds;
        if !(least.is_finite() && most.is_finite() && 0. <= least && least <= most && most <= 2.) {
            return Err("Processing needs an ordered range within 0 to 2 seconds".into());
        }
        self.congestion.validate()
    }
}

/// Episodes in which a connection's round trip rises, then recovers at once.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct Congestion {
    /// Mean episode starts per connection-second. Each connection draws its own
    /// rate exponentially around it; zero disables congestion.
    pub rate_per_second: f64,
    /// Mean episode length; lengths are exponential.
    pub duration_seconds: f64,
    /// Chance that another episode starts an exponential `follow_seconds` (mean)
    /// after one ends.
    pub follow_probability: f64,
    pub follow_seconds: f64,
    /// Extra round trip: Pareto from the lower bound with `shape`, capped at the upper.
    pub extra_seconds: [f64; 2],
    pub shape: f64,
}
impl Default for Congestion {
    fn default() -> Self {
        Self {
            rate_per_second: 0.,
            duration_seconds: 0.,
            follow_probability: 0.,
            follow_seconds: 0.,
            extra_seconds: [0.; 2],
            shape: 1.,
        }
    }
}
impl Congestion {
    fn validate(&self) -> Result<(), String> {
        let within = |value: f64, max: f64| value.is_finite() && (0. ..=max).contains(&value);
        let [least, most] = self.extra_seconds;
        if !within(self.rate_per_second, 1.)
            || !within(self.duration_seconds, 10.)
            || !within(self.follow_probability, 1.)
            || !within(self.follow_seconds, 10.)
            || !within(least, most)
            || !within(most, 2.)
            || !self.shape.is_finite()
            || !(0.1..=100.).contains(&self.shape)
        {
            return Err(
                "Congestion needs a rate up to 1/s, durations and follow-ups up to 10 s, \
                a follow-up chance up to 1, ordered extra round trips up to 2 s and a \
                shape from 0.1 to 100"
                    .into(),
            );
        }
        Ok(())
    }
}

/// Seeded xorshift stream; one per backend so runs are reproducible.
#[derive(Clone, Debug)]
pub(crate) struct Random(u64);
impl Random {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed | 1)
    }
    pub(crate) fn unit(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn exponential(&mut self, mean: f64) -> f64 {
        -mean * (1. - self.unit()).ln()
    }
}

/// One connection's private draws and stream clocks. Each direction is one TCP
/// stream: a message never arrives before one sent earlier in that direction.
#[derive(Clone, Debug, Default)]
pub(crate) struct Link {
    round_trip: f64,
    congestion_rate: f64,
    processing: f64,
    congestion: f64,
    congested_until: f64,
    follow_at: Option<f64>,
    updated_at: Option<f64>,
    uplink_last: f64,
    downlink_last: f64,
}
impl Link {
    /// A new connection with its own base round trip, congestion rate and processing.
    pub(crate) fn connect(config: &LatencyConfig, random: &mut Random) -> Self {
        let unit = random.unit();
        let weights = &config.round_trip_weights;
        let total = if weights.is_empty() {
            config.round_trip_seconds.len() as f64
        } else {
            weights.iter().sum()
        };
        let mut left = unit * total;
        let mut picked = 0.;
        for (index, &round_trip) in config.round_trip_seconds.iter().enumerate() {
            let weight = weights.get(index).copied().unwrap_or(1.);
            if weight > 0. {
                picked = round_trip;
                if left < weight {
                    break;
                }
                left -= weight;
            }
        }
        let spread = config.round_trip_spread;
        let normal = if spread > 0. {
            (-2. * (1. - random.unit()).ln()).sqrt() * (std::f64::consts::TAU * random.unit()).cos()
        } else {
            0.
        };
        let round_trip = (picked * (spread * normal).exp()).min(2.);
        let congestion_rate = random.exponential(config.congestion.rate_per_second);
        let [least, most] = config.processing_seconds;
        let processing = if most > least {
            least + random.unit() * (most - least)
        } else {
            least
        };
        Self {
            round_trip,
            congestion_rate,
            processing,
            ..Default::default()
        }
    }
    /// Start or end congestion episodes up to `now`; call at each broadcast boundary.
    pub(crate) fn update(&mut self, config: &LatencyConfig, random: &mut Random, now: f64) {
        let elapsed = self.updated_at.map_or(0., |at| (now - at).max(0.));
        self.updated_at = Some(now);
        let congestion = config.congestion;
        if now < self.congested_until {
            return;
        }
        if self.congestion > 0. {
            let follows = random.unit() < congestion.follow_probability;
            self.follow_at = follows.then(|| now + random.exponential(congestion.follow_seconds));
            self.congestion = 0.;
        }
        let rate = self.congestion_rate;
        let followed = self.follow_at.is_some_and(|at| at <= now);
        if followed || (rate > 0. && random.unit() < 1. - (-rate * elapsed).exp()) {
            let duration = random.exponential(congestion.duration_seconds);
            let [least, most] = congestion.extra_seconds;
            let extra = least * (1. - random.unit()).powf(-1. / congestion.shape);
            self.follow_at = None;
            self.congested_until = now + duration;
            self.congestion = extra.min(most);
        }
    }
    fn delay(&self, config: &LatencyConfig, random: &mut Random, share: f64) -> f64 {
        let base = (self.round_trip + self.congestion) * share;
        (base + (2. * random.unit() - 1.) * config.jitter_seconds).max(0.)
    }
    /// Arrival time at the server of a message the client sends at `sent`.
    pub(crate) fn uplink(&mut self, config: &LatencyConfig, random: &mut Random, sent: f64) -> f64 {
        let delay = self.delay(config, random, config.uplink_share);
        self.uplink_last = (sent + delay).max(self.uplink_last);
        self.uplink_last
    }
    /// Delivery time at the client of a frame the server emits at `emitted`.
    pub(crate) fn downlink(
        &mut self,
        config: &LatencyConfig,
        random: &mut Random,
        emitted: f64,
    ) -> f64 {
        let delay = self.delay(config, random, 1. - config.uplink_share);
        self.downlink_last = (emitted + delay).max(self.downlink_last);
        self.downlink_last
    }
    /// Processing time of one reply: the connection's base with per-reply jitter.
    pub(crate) fn processing(&self, config: &LatencyConfig, random: &mut Random) -> f64 {
        let jitter = config.processing_jitter_seconds;
        if jitter > 0. {
            (self.processing + (2. * random.unit() - 1.) * jitter).max(0.)
        } else {
            self.processing
        }
    }
}
