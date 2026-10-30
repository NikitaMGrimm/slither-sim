//! Native host demand only. FIFO acquisition supplies the refresh backpressure;
//! this owner never changes LocalGame's clock, update or acknowledgement rules.
use std::time::{Duration, Instant};

// Recovery only, never the normal frame cadence. A surface that immediately
// refuses acquisition must not turn request_redraw into a hot loop.
const RETRY_DELAY: Duration = Duration::from_millis(100);

pub struct Schedule {
    active: bool,
    visible: bool,
    dirty: bool,
    requested: bool,
    retry: Option<Instant>,
}
impl Default for Schedule {
    fn default() -> Self {
        Self {
            active: false,
            visible: true,
            dirty: true,
            requested: false,
            retry: None,
        }
    }
}
impl Schedule {
    pub fn activity(&mut self, active: bool) {
        if self.active != active {
            self.invalidate();
        }
        self.active = active;
    }
    pub fn visibility(&mut self, visible: bool) {
        if self.visible != visible {
            self.invalidate();
        }
        self.visible = visible;
        if !visible {
            // Some window systems discard queued redraws while minimized.
            self.requested = false;
        }
    }
    pub fn invalidate(&mut self) {
        self.dirty = true;
    }
    fn ready(&self, now: Instant) -> bool {
        self.visible && (self.active || self.dirty) && self.retry.is_none_or(|t| now >= t)
    }
    pub fn request_redraw(&mut self, now: Instant) -> bool {
        if self.requested || !self.ready(now) {
            return false;
        }
        self.requested = true;
        true
    }
    pub fn begin_redraw(&mut self, now: Instant) -> bool {
        let requested = std::mem::take(&mut self.requested);
        if !requested || !self.ready(now) {
            return false;
        }
        self.dirty = false;
        true
    }
    pub fn presented(&mut self) {
        self.retry = None;
    }
    pub fn retry(&mut self, now: Instant) {
        self.dirty = true;
        self.retry = Some(now + RETRY_DELAY);
    }
    pub fn deadline(&self) -> Option<Instant> {
        self.retry
            .filter(|_| !self.requested && self.visible && (self.active || self.dirty))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn present_requests_one_successor_and_duplicate_redraw_does_not_advance() {
        let now = Instant::now();
        let mut s = Schedule::default();
        s.activity(true);
        for _ in 0..1000 {
            assert!(s.request_redraw(now));
            assert!(!s.request_redraw(now));
            assert!(s.begin_redraw(now));
            assert!(!s.begin_redraw(now));
            s.presented();
        }
    }
    #[test]
    fn idle_draws_once_then_waits_until_invalidated() {
        let now = Instant::now();
        let mut s = Schedule::default();
        assert!(s.request_redraw(now));
        assert!(s.begin_redraw(now));
        s.presented();
        assert!(!s.request_redraw(now));
        s.invalidate(); // resize / explicit Play / Stop / OS expose
        assert!(s.request_redraw(now));
        assert!(s.begin_redraw(now));
    }
    #[test]
    fn every_visibility_activity_request_and_retry_combination() {
        let now = Instant::now();
        for active in [false, true] {
            for visible in [false, true] {
                for dirty in [false, true] {
                    for requested in [false, true] {
                        for retry in [None, Some(now), Some(now + RETRY_DELAY)] {
                            let mut s = Schedule {
                                active,
                                visible,
                                dirty,
                                requested,
                                retry,
                            };
                            let ready =
                                visible && (active || dirty) && retry.is_none_or(|t| t <= now);
                            assert_eq!(s.request_redraw(now), !requested && ready);
                            assert_eq!(s.begin_redraw(now), (requested || ready) && ready);
                            assert!(!s.begin_redraw(now));
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn occlusion_and_zero_resize_cancel_queued_draw_and_resume_once() {
        let now = Instant::now();
        let mut s = Schedule::default();
        s.activity(true);
        assert!(s.request_redraw(now));
        s.visibility(false);
        assert!(!s.begin_redraw(now));
        assert!(!s.request_redraw(now));
        assert_eq!(s.deadline(), None);
        s.visibility(true);
        assert!(s.request_redraw(now));
        assert!(!s.request_redraw(now));
    }
    #[test]
    fn immediate_surface_failure_waits_and_resize_cannot_bypass_retry() {
        let now = Instant::now();
        let mut s = Schedule::default();
        s.activity(true);
        assert!(s.request_redraw(now));
        assert!(s.begin_redraw(now));
        s.retry(now);
        s.invalidate();
        assert!(!s.request_redraw(now));
        assert_eq!(s.deadline(), Some(now + RETRY_DELAY));
        assert!(s.request_redraw(now + RETRY_DELAY));
        assert!(s.begin_redraw(now + RETRY_DELAY));
        s.presented();
        assert_eq!(s.deadline(), None);
    }
    #[test]
    fn restore_rearms_even_if_the_os_discarded_the_queued_redraw() {
        let now = Instant::now();
        let mut s = Schedule::default();
        s.activity(true);
        assert!(s.request_redraw(now));
        s.visibility(false);
        assert!(!s.request_redraw(now));
        // No RedrawRequested was delivered while minimized.
        s.visibility(true);
        assert!(s.request_redraw(now));
        assert!(s.begin_redraw(now));
        s.presented();
        s.activity(false);
        assert!(s.request_redraw(now));
        assert!(s.begin_redraw(now));
        s.presented();
        assert!(!s.request_redraw(now));
    }
}
