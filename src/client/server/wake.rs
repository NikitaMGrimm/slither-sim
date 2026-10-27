//! Windows socket wakeup: output notification plus a high-resolution poll limit.
//!
//! A channel's millisecond timeout was measured to delay incoming socket reads
//! by ~15 ms on Windows. Keep the timer local to this wait; changing the global
//! timer resolution or spinning would impose cost on unrelated threads.
//! See research/fidelity/2026-10-27-host-frame-timing/REPORT.md.
use std::{ffi::c_void, io, ptr, time::Duration};

type Handle = *mut c_void;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(
        attributes: *const c_void,
        manual: i32,
        initial: i32,
        name: *const u16,
    ) -> Handle;
    fn CreateWaitableTimerExW(
        attributes: *const c_void,
        name: *const u16,
        flags: u32,
        access: u32,
    ) -> Handle;
    fn SetEvent(event: Handle) -> i32;
    fn SetWaitableTimerEx(
        timer: Handle,
        due: *const i64,
        period: i32,
        completion: *const c_void,
        argument: *const c_void,
        context: *const c_void,
        tolerance: u32,
    ) -> i32;
    fn WaitForMultipleObjects(count: u32, handles: *const Handle, all: i32, timeout: u32) -> u32;
    fn CloseHandle(handle: Handle) -> i32;
}

struct OwnedHandle(Handle);

impl OwnedHandle {
    fn new(handle: Handle) -> io::Result<Self> {
        if handle.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(handle))
        }
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: this owns a live kernel handle, closed only here.
        unsafe { CloseHandle(self.0) };
    }
}

pub(super) struct Wake {
    event: OwnedHandle,
    timer: OwnedHandle,
}

// SAFETY: kernel event/timer operations are thread-safe. Shared owns both handles
// until every host thread has joined. Only the socket thread arms/waits the timer;
// the simulation thread signals the event after publishing a channel batch.
unsafe impl Send for Wake {}
unsafe impl Sync for Wake {}

impl Wake {
    pub(super) fn new() -> io::Result<Self> {
        // Auto-reset event retains one notification even before the wait starts.
        // SAFETY: null optional pointers, no names/security attributes retained.
        let event = OwnedHandle::new(unsafe { CreateEventW(ptr::null(), 0, 0, ptr::null()) })?;
        // CREATE_WAITABLE_TIMER_HIGH_RESOLUTION; SYNCHRONIZE | TIMER_MODIFY_STATE.
        // SAFETY: as above; the returned timer is owned independently of the event.
        let timer = OwnedHandle::new(unsafe {
            CreateWaitableTimerExW(ptr::null(), ptr::null(), 2, 0x0010_0002)
        })
        .or_else(|error| {
            // The high-resolution flag needs Windows 10 1803. Preserve host
            // startup on older supported Windows when that flag is rejected.
            if error.raw_os_error() != Some(87) {
                return Err(error);
            }
            // SAFETY: the same valid arguments, without the unsupported flag.
            OwnedHandle::new(unsafe {
                CreateWaitableTimerExW(ptr::null(), ptr::null(), 0, 0x0010_0002)
            })
        })?;
        Ok(Self { event, timer })
    }

    pub(super) fn notify(&self) -> io::Result<()> {
        // SAFETY: the event stays live for this call, including across threads.
        if unsafe { SetEvent(self.event.0) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    pub(super) fn wait(&self, limit: Duration) -> io::Result<()> {
        // Negative 100 ns units are a relative deadline. Round upward so the
        // timeout cannot poll faster than requested, even for sub-100 ns limits.
        let due = -(limit.as_nanos().div_ceil(100).clamp(1, i64::MAX as u128) as i64);
        // SAFETY: valid timer and due pointer, no callback or retained pointers.
        if unsafe {
            SetWaitableTimerEx(
                self.timer.0,
                &due,
                0,
                ptr::null(),
                ptr::null(),
                ptr::null(),
                0,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let handles = [self.event.0, self.timer.0];
        // SAFETY: both handles remain live through this non-alertable wait.
        match unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, u32::MAX) } {
            0 | 1 => Ok(()),
            _ => Err(io::Error::last_os_error()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::Arc, thread, time::Instant};

    #[test]
    fn notification_survives_until_wait_and_timer_rearms() {
        let wake = Arc::new(Wake::new().unwrap());
        wake.notify().unwrap();
        let at = Instant::now();
        wake.wait(Duration::from_secs(1)).unwrap();
        assert!(at.elapsed() < Duration::from_millis(500));
        // Consuming the event must leave the next wait bounded by its timer.
        let at = Instant::now();
        wake.wait(Duration::from_millis(5)).unwrap();
        assert!(at.elapsed() >= Duration::from_millis(5));
        let sender = wake.clone();
        let thread = thread::spawn(move || sender.notify().unwrap());
        let at = Instant::now();
        wake.wait(Duration::from_secs(1)).unwrap();
        assert!(at.elapsed() < Duration::from_millis(500));
        thread.join().unwrap();
    }
}
