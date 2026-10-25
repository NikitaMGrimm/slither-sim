//! Simulated network conditions between each client and the server.
//!
//! Each connection draws a base round trip. Each direction is an ordered stream:
//! messages add right-skewed variation, partly correlated over seconds, and wait
//! out short pauses, releasing held frames together. Replies incur processing
//! time. Delays use continuous seconds; arrivals apply on physics ticks. Clients
//! measure only pings and their own reply time.
//!
//! The model and `LatencyConfig::measured_route` come from the [network path
//! study] of retained official captures. It fits effective application
//! delivery on one low-RTT route; uplink behavior, direction coupling and
//! physical causes are not identified.
//!
//! [network path study]: https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-25-network-path/REPORT.md
use serde::{Deserialize, Serialize};

/// `Random::unit` is a multiple of 2^-53 below 1, so `1 - unit` is at least
/// 2^-53: an exponential draw is at most 53 ln 2 (about 36.7) times its mean,
/// and a Box-Muller normal at most the square root of twice that (about 8.57).
fn max_exponential() -> f64 {
    53. * std::f64::consts::LN_2
}
fn max_normal() -> f64 {
    (2. * max_exponential()).sqrt()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct LatencyConfig {
    /// Base round trips, picked by `round_trip_weights` (empty: equally) once
    /// per connection. An empty list gives a zero base round trip.
    pub round_trip_seconds: Vec<f64>,
    pub round_trip_weights: Vec<f64>,
    /// Standard deviation of the log base round trip around the picked value.
    pub round_trip_spread: f64,
    /// Share of every base round trip spent on the uplink.
    pub uplink_share: f64,
    /// Extra delay of each message beyond its base share.
    pub variation: Variation,
    /// Holds of each direction's stream.
    pub pauses: Pauses,
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
            variation: Variation::default(),
            pauses: Pauses::default(),
            processing_seconds: [0.; 2],
            processing_jitter_seconds: 0.,
        }
    }
}
impl LatencyConfig {
    /// A fixed round trip split evenly, without variation or pauses.
    pub fn fixed(round_trip_seconds: f64) -> Self {
        Self {
            round_trip_seconds: vec![round_trip_seconds],
            ..Default::default()
        }
    }
    /// Fixed one-way delays, without variation or pauses.
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
    /// The retained low-RTT official route, as fitted and validated on held-out
    /// sessions by the network path study. Its 4.97 ms base round trip assumes
    /// the server answers within one tick of arrival, as this simulator does;
    /// with that the game ping median is near the route's 13.4 ms. The even
    /// split and mirrored uplink are assumptions. Add it only to paths that
    /// lack real delay: a remote player already has their own.
    pub fn measured_route() -> Self {
        Self {
            round_trip_seconds: vec![0.004974],
            variation: Variation {
                median_seconds: 0.001901,
                log_sigma: 0.5066,
                correlated_share: 0.1771,
                correlation_seconds: 1.837,
            },
            pauses: Pauses {
                rate_per_second: 1.646,
                mean_seconds: 0.01393,
            },
            ..Default::default()
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self
            .round_trip_seconds
            .iter()
            .chain([&self.processing_jitter_seconds])
            .any(|v| !v.is_finite() || *v < 0.)
        {
            return Err("Latency must be finite and nonnegative".into());
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
        if !self.round_trip_spread.is_finite() || self.round_trip_spread < 0. {
            return Err("Round-trip spread must be finite and nonnegative".into());
        }
        if !(0. ..=1.).contains(&self.uplink_share) {
            return Err("Uplink share must be from 0 to 1".into());
        }
        let [least, most] = self.processing_seconds;
        if !(least.is_finite() && most.is_finite() && 0. <= least && least <= most) {
            return Err("Processing needs an ordered finite nonnegative range".into());
        }
        self.variation.validate()?;
        self.pauses.validate()?;
        // Delays are added to clocks; the largest possible draw must be finite.
        let base = self.round_trip_seconds.iter().copied().fold(0., f64::max);
        let round_trip = base * (self.round_trip_spread * max_normal()).exp();
        if !(round_trip + self.variation.largest()).is_finite() {
            return Err(
                "Largest delay (roundTripSeconds × exp(8.57 × roundTripSpread) + \
                 medianSeconds × exp(12.1 × logSigma)) must be finite"
                    .into(),
            );
        }
        if !(most + self.processing_jitter_seconds).is_finite() {
            return Err("processingSeconds + processingJitterSeconds must be finite".into());
        }
        Ok(())
    }
}

/// Lognormal extra delay of each message,
/// `median × exp(logSigma × (√c·X + √(1−c)·Z))` with `c = correlatedShare`:
/// `Z` is fresh per message and `X` a per-direction Ornstein–Uhlenbeck state of
/// unit variance that decorrelates over `correlationSeconds`, so a connection
/// has slow good and bad spells under fast noise. A zero median adds nothing.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct Variation {
    pub median_seconds: f64,
    pub log_sigma: f64,
    pub correlated_share: f64,
    pub correlation_seconds: f64,
}
impl Variation {
    fn validate(&self) -> Result<(), String> {
        let finite = |v: f64| v.is_finite() && v >= 0.;
        if !finite(self.median_seconds)
            || !finite(self.log_sigma)
            || !(0. ..=1.).contains(&self.correlated_share)
            || !finite(self.correlation_seconds)
            || self.correlated_share > 0. && self.correlation_seconds == 0.
        {
            return Err(
                "Variation needs a finite nonnegative median, log sigma and correlation \
                 time, a correlated share from 0 to 1, and a positive correlation time \
                 when that share is positive"
                    .into(),
            );
        }
        Ok(())
    }
    /// Largest draw: `√c·X + √(1−c)·Z` is at most `√2 × 8.57`, about 12.1.
    fn largest(&self) -> f64 {
        let normal = std::f64::consts::SQRT_2 * max_normal();
        if self.median_seconds > 0. {
            self.median_seconds * (self.log_sigma * normal).exp()
        } else {
            0.
        }
    }
}

/// Delivery pauses of each direction: starts are Poisson at `ratePerSecond`,
/// lengths exponential with mean `meanSeconds`, and overlapping pauses merge.
/// A message due inside a pause waits for its end, with everything behind it.
/// A zero rate or mean disables pauses.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct Pauses {
    pub rate_per_second: f64,
    pub mean_seconds: f64,
}
impl Pauses {
    fn enabled(&self) -> bool {
        self.rate_per_second > 0. && self.mean_seconds > 0.
    }
    fn validate(&self) -> Result<(), String> {
        // Exponential draws scale their means by at most `max_exponential`.
        let draw = |mean: f64| mean.is_finite() && (mean * max_exponential()).is_finite();
        if !(self.rate_per_second.is_finite() && self.rate_per_second >= 0.)
            || !(self.mean_seconds >= 0. && draw(self.mean_seconds))
            || self.rate_per_second > 0. && !draw(1. / self.rate_per_second)
        {
            return Err(
                "Pauses need a finite nonnegative rate and mean whose largest exponential \
                 draws (36.7 × mean, and 36.7 ÷ rate between starts) are finite"
                    .into(),
            );
        }
        // Each start costs a step, and starts closer than the host's 1 ms
        // delivery resolution cannot be observed.
        if self.rate_per_second > MAX_PAUSE_RATE {
            return Err(format!(
                "Pauses need ratePerSecond ≤ {MAX_PAUSE_RATE}; delivery cannot resolve finer pauses"
            ));
        }
        // Merged pauses last (e^(rate × mean) − 1) / rate on average: past one
        // they quickly cover nearly all time, and the stream no longer models
        // delivery delays but an outage.
        if self.rate_per_second * self.mean_seconds > 1. {
            return Err(
                "Pauses need ratePerSecond × meanSeconds ≤ 1; more would merge into \
                 near-permanent holds"
                    .into(),
            );
        }
        Ok(())
    }
}

/// Pause starts per second and direction. A processing bound, not a measured
/// limit: the fitted official route has 1.646.
const MAX_PAUSE_RATE: f64 = 1000.;

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
    fn normal(&mut self) -> f64 {
        (-2. * (1. - self.unit()).ln()).sqrt() * (std::f64::consts::TAU * self.unit()).cos()
    }
}

/// One ordered TCP direction: messages cannot overtake earlier messages.
#[derive(Clone, Debug, Default)]
struct Stream {
    last: f64,
    /// Ornstein–Uhlenbeck state and when it was last advanced.
    state: f64,
    state_at: Option<f64>,
    /// The latest merged pause and the next pause start.
    pause: (f64, f64),
    next_pause: Option<f64>,
}
impl Stream {
    fn deliver(&mut self, config: &LatencyConfig, random: &mut Random, base: f64, at: f64) -> f64 {
        let due = at + base + self.variation(&config.variation, random, at);
        let held = self.hold(&config.pauses, random, due);
        self.last = self.last.max(held);
        self.last
    }

    fn variation(&mut self, variation: &Variation, random: &mut Random, at: f64) -> f64 {
        if variation.median_seconds == 0. {
            return 0.;
        }
        let share = variation.correlated_share;
        let slow = if share > 0. {
            // A new stream starts from the stationary distribution.
            self.state = match self.state_at {
                None => random.normal(),
                Some(before) => {
                    let decay = (-(at - before).max(0.) / variation.correlation_seconds).exp();
                    decay * self.state + (1. - decay * decay).sqrt() * random.normal()
                }
            };
            self.state_at = Some(at);
            self.state
        } else {
            0.
        };
        let normal = share.sqrt() * slow + (1. - share).sqrt() * random.normal();
        variation.median_seconds * (variation.log_sigma * normal).exp()
    }

    /// When a message due at `due` leaves the pauses. Pauses are generated in
    /// start order; only the latest merged one matters, since an earlier pause
    /// ended before a message already delivered.
    fn hold(&mut self, pauses: &Pauses, random: &mut Random, due: f64) -> f64 {
        if !pauses.enabled() {
            return due;
        }
        let gap = 1. / pauses.rate_per_second;
        let mut next = *self
            .next_pause
            .get_or_insert_with(|| due + random.exponential(gap));
        loop {
            let (start, end) = self.pause;
            let inside = start <= due && due < end;
            // Starts up to `due`, and starts overlapping the pause holding it.
            if next > due && !(inside && next <= end) {
                break;
            }
            let length = random.exponential(pauses.mean_seconds);
            self.pause = if next <= end {
                (start, end.max(next + length))
            } else {
                (next, next + length)
            };
            next += random.exponential(gap);
        }
        self.next_pause = Some(next);
        let (start, end) = self.pause;
        if start <= due && due < end { end } else { due }
    }

    /// Move every stored time `seconds` earlier.
    fn shift(&mut self, seconds: f64) {
        self.last -= seconds;
        self.state_at = self.state_at.map(|at| at - seconds);
        self.pause = (self.pause.0 - seconds, self.pause.1 - seconds);
        self.next_pause = self.next_pause.map(|at| at - seconds);
    }
}

/// One connection's private draws and streams.
#[derive(Clone, Debug, Default)]
pub(crate) struct Link {
    round_trip: f64,
    processing: f64,
    uplink: Stream,
    downlink: Stream,
}
impl Link {
    /// A new connection with its own base round trip and processing.
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
        let normal = if spread > 0. { random.normal() } else { 0. };
        let round_trip = picked * (spread * normal).exp();
        let [least, most] = config.processing_seconds;
        let processing = if most > least {
            least + random.unit() * (most - least)
        } else {
            least
        };
        Self {
            round_trip,
            processing,
            ..Default::default()
        }
    }
    /// Arrival time at the server of a message the client sends at `sent`.
    pub(crate) fn uplink(&mut self, config: &LatencyConfig, random: &mut Random, sent: f64) -> f64 {
        let base = self.round_trip * config.uplink_share;
        self.uplink.deliver(config, random, base, sent)
    }
    /// Delivery time at the client of a frame the server emits at `emitted`.
    pub(crate) fn downlink(
        &mut self,
        config: &LatencyConfig,
        random: &mut Random,
        emitted: f64,
    ) -> f64 {
        let base = self.round_trip * (1. - config.uplink_share);
        self.downlink.deliver(config, random, base, emitted)
    }
    /// Move both streams' clocks `seconds` earlier; see `RawSimulator::shift_links`.
    pub(crate) fn shift(&mut self, seconds: f64) {
        self.uplink.shift(seconds);
        self.downlink.shift(seconds);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Deliveries of messages sent every `step` seconds for `seconds`.
    fn deliveries(config: &LatencyConfig, step: f64, seconds: f64) -> Vec<(f64, f64)> {
        let mut random = Random::new(5);
        let mut link = Link::connect(config, &mut random);
        (0..(seconds / step) as usize)
            .map(|i| {
                let sent = i as f64 * step;
                (sent, link.downlink(config, &mut random, sent))
            })
            .collect()
    }

    #[test]
    fn measured_route_reproduces_its_fitted_delay_shape() {
        let config = LatencyConfig::measured_route();
        config.validate().unwrap();
        let mut extra: Vec<f64> = deliveries(&config, 0.007, 600.)
            .iter()
            .map(|(sent, at)| at - sent - 0.004974 / 2.)
            .collect();
        extra.sort_by(f64::total_cmp);
        let quantile = |p: f64| extra[(p * extra.len() as f64) as usize];
        // Ordering and pauses add to the lognormal median of 1.9 ms; the held-out
        // official median-session p50/p90 are 1.6/3.3 ms and the fitted model's
        // 1.6/3.5 ms, measured over a rolling floor this direct draw lacks.
        assert!(
            (0.0017..0.0026).contains(&quantile(0.5)),
            "{}",
            quantile(0.5)
        );
        assert!(
            (0.003..0.0055).contains(&quantile(0.9)),
            "{}",
            quantile(0.9)
        );
        // Pauses of ~14 ms hold about 2% of messages well past the lognormal.
        let held = extra.iter().filter(|&&e| e > 0.010).count() as f64 / extra.len() as f64;
        assert!((0.005..0.04).contains(&held), "{held}");
    }

    #[test]
    fn streams_stay_ordered_and_pauses_release_together() {
        let config = LatencyConfig {
            pauses: Pauses {
                rate_per_second: 2.,
                mean_seconds: 0.05,
            },
            ..LatencyConfig::measured_route()
        };
        let delivered = deliveries(&config, 0.007, 120.);
        assert!(delivered.windows(2).all(|w| w[0].1 <= w[1].1));
        // Messages held by one pause share its end.
        let together = delivered.windows(2).filter(|w| w[0].1 == w[1].1).count();
        assert!(together > 100, "{together}");
        assert!(delivered.iter().all(|(sent, at)| at >= sent));
    }

    #[test]
    fn disabled_parts_add_nothing() {
        let config = LatencyConfig::fixed(0.02);
        assert!(
            deliveries(&config, 0.007, 10.)
                .iter()
                .all(|(sent, at)| (at - sent - 0.01).abs() < 1e-12)
        );
    }

    #[test]
    fn a_shifted_link_delivers_as_if_its_times_never_moved() {
        let config = LatencyConfig {
            round_trip_seconds: vec![0.02],
            variation: Variation {
                median_seconds: 0.05,
                log_sigma: 2.,
                correlated_share: 0.5,
                correlation_seconds: 1.,
            },
            pauses: Pauses {
                rate_per_second: 5.,
                mean_seconds: 0.1,
            },
            ..Default::default()
        };
        let mut random = Random::new(152);
        let mut link = Link::connect(&config, &mut random);
        link.uplink(&config, &mut random, 0.14);
        link.downlink(&config, &mut random, 0.14);
        let (mut shifted, mut shifted_random) = (link.clone(), random.clone());
        shifted.shift(0.7);
        for at in [0.147, 1.106, 1.113, 1.5] {
            let up = link.uplink(&config, &mut random, at);
            let down = link.downlink(&config, &mut random, at);
            let up_shifted = shifted.uplink(&config, &mut shifted_random, at - 0.7);
            let down_shifted = shifted.downlink(&config, &mut shifted_random, at - 0.7);
            assert!((up - 0.7 - up_shifted).abs() < 1e-9, "{up} {up_shifted}");
            assert!(
                (down - 0.7 - down_shifted).abs() < 1e-9,
                "{down} {down_shifted}"
            );
        }
    }

    #[test]
    fn pause_rates_delivery_cannot_resolve_are_rejected() {
        let pauses = |rate_per_second| Pauses {
            rate_per_second,
            mean_seconds: 1e-21,
        };
        assert!(pauses(MAX_PAUSE_RATE).validate().is_ok());
        // Such starts would round onto one instant and never advance.
        assert!(pauses(1e20).validate().is_err());
    }

    #[test]
    fn pause_occupancy_above_one_is_rejected() {
        let mut config = LatencyConfig::measured_route();
        config.pauses.rate_per_second = 100.;
        config.pauses.mean_seconds = 0.1;
        assert!(config.validate().unwrap_err().contains("≤ 1"));
        config.pauses.mean_seconds = 0.01;
        config.validate().unwrap();
    }
}
