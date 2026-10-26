use super::{Bot, CONTRACT, ClientFrame, ClientStatus, ReceivedField, ReceivedMetadata};

/// Received-frame adapter for [`crate::bot::Forager`].
///
/// It derives the forecast lead from the frame's own timing. The observation
/// is as old as the latest received message (the smallest valid receipt age of
/// any snake position/heading, body or food report) plus the downlink share of
/// the measured round trip. A command sent now acts after the uplink share plus
/// the previous decision's processing time. The recent ping range is the
/// lead's uncertainty. Each opponent's latest reported wanted heading and
/// heading-receipt age come from its `received.snakes` row. Only received
/// fields are read; positions between reports are already extrapolated by the
/// decoder. The bot keeps reusable buffers but no state between decisions, so
/// `reset` has nothing to clear.
#[derive(Clone, Debug)]
pub struct ForagerBot {
    policy: crate::bot::Forager,
    intents: Vec<crate::bot::Intent>,
}

impl ForagerBot {
    pub fn new(policy: crate::bot::PolicyId) -> Self {
        Self {
            policy: crate::bot::Forager::new(policy),
            intents: Vec::with_capacity(4096),
        }
    }
    /// The kernel timing this adapter derives from a frame.
    pub fn timing(&self, frame: &ClientFrame) -> crate::bot::Timing {
        let timing = &frame.timing;
        let ping = timing.ping_seconds.unwrap_or(0.0);
        let uplink = self.policy.uplink_share();
        let received = &frame.received;
        let age = |field: ReceivedField| field.valid.then_some(field.age_seconds);
        let receipt = received
            .snakes
            .iter()
            .flat_map(|row| [age(row.position), age(row.heading)])
            .chain([
                received.bodies.valid.then_some(received.bodies.age_seconds),
                received.food.valid.then_some(received.food.age_seconds),
            ])
            .flatten()
            .fold(f64::INFINITY, f64::min);
        let receipt = if receipt.is_finite() { receipt } else { 0.0 };
        let mut result = crate::bot::Timing {
            observation_age: ping * (1.0 - uplink) + receipt,
            command_delay: ping * uplink,
            processing: timing.processing_seconds,
            uncertainty: timing
                .ping_range_seconds
                .map_or(0.0, |[low, high]| high - low),
            ..Default::default()
        };
        for command in &timing.sent_commands {
            result.push_sent(command.seconds_ago, command.action);
        }
        result
    }
}

impl Bot for ForagerBot {
    fn decide(&mut self, frame: &ClientFrame) -> Option<Action> {
        if frame.contract != CONTRACT || frame.status != ClientStatus::Playing {
            return None;
        }
        let observation = frame.observation.as_ref()?;
        let timing = self.timing(frame);
        fill_intents(&mut self.intents, observation, &frame.received);
        Some(self.policy.decide(observation, &timing, &self.intents))
    }
}

/// Per-opponent received knowledge in observation order. Opponents and
/// receipt rows both follow the decoder's slot order, so the scan resumes after
/// the previous match and usually finds the next row at once.
fn fill_intents(
    intents: &mut Vec<crate::bot::Intent>,
    observation: &crate::Observation,
    received: &ReceivedMetadata,
) {
    intents.clear();
    let rows = &received.snakes;
    let mut next = 0;
    for opponent in &observation.opponents {
        let mut intent = crate::bot::Intent::default();
        let found = (0..rows.len())
            .map(|i| (next + i) % rows.len())
            .find(|&i| rows[i].snake_id == opponent.id);
        if let Some(i) = found {
            next = i + 1;
            let row = &rows[i];
            if let Some(wanted) = row.wanted_history.first() {
                // Motion reports round the target to 256 steps; the server
                // steers to the client's 251-step command, so take the nearest.
                let step = std::f64::consts::TAU / 251.0;
                intent.wanted =
                    (wanted.value.rem_euclid(std::f64::consts::TAU) / step).round() % 251.0 * step;
            }
            if row.heading.valid {
                intent.heading_age = row.heading.age_seconds;
            }
        }
        intents.push(intent);
    }
}

#[cfg(test)]
mod conformance;
#[cfg(test)]
mod tests;
