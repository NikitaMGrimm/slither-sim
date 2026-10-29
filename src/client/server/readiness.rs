//! Linux readiness snapshot; never waits or changes delivery/flush clocks.
use std::{
    ffi::{c_int, c_short, c_ulong},
    os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
};

#[repr(C)]
struct PollFd {
    fd: c_int,
    events: c_short,
    revents: c_short,
}

// Linux packs this ABI on x86; other targets retain their C alignment.
#[repr(C)]
#[cfg_attr(
    any(target_arch = "x86_64", all(target_arch = "x86", target_env = "gnu")),
    repr(packed)
)]
#[derive(Clone, Copy, Default)]
struct Event {
    events: u32,
    index: u64,
}

unsafe extern "C" {
    fn poll(fds: *mut PollFd, count: c_ulong, timeout: c_int) -> c_int;
    fn epoll_create1(flags: c_int) -> c_int;
    fn epoll_ctl(epoll: c_int, operation: c_int, fd: c_int, event: *const Event) -> c_int;
    fn epoll_wait(epoll: c_int, events: *mut Event, count: c_int, timeout: c_int) -> c_int;
}

pub(super) struct Readiness {
    fds: Vec<PollFd>,
    events: Vec<Event>,
    epoll: Option<OwnedFd>,
    failed: bool,
}

impl Default for Readiness {
    fn default() -> Self {
        // Linux EPOLL_CLOEXEC has the SPARC-specific O_CLOEXEC value.
        let flags = if cfg!(any(target_arch = "sparc", target_arch = "sparc64")) {
            0x400000
        } else {
            0x80000
        };
        let fd = unsafe { epoll_create1(flags) };
        Self {
            fds: Vec::new(),
            events: Vec::new(),
            // A successful create transfers this descriptor's ownership here.
            epoll: (fd >= 0).then(|| unsafe { OwnedFd::from_raw_fd(fd) }),
            failed: false,
        }
    }
}

impl Readiness {
    pub(super) fn poll<'a>(
        &mut self,
        sockets: impl Iterator<Item = Option<(RawFd, &'a mut Option<usize>)>>,
    ) {
        self.fds.clear();
        let mut registered = self.epoll.is_some();
        for socket in sockets {
            let index = self.fds.len();
            let fd = if let Some((fd, previous)) = socket {
                if *previous != Some(index)
                    && let Some(epoll) = &self.epoll
                {
                    // ADD is tied to an Open's lifetime, not its fd number:
                    // close removes the old watch even when that number is reused.
                    // Compaction only needs MOD to update the event's index.
                    let operation = if previous.is_none() { 1 } else { 3 };
                    let event = Event {
                        events: 1,
                        index: index as u64,
                    };
                    if unsafe { epoll_ctl(epoll.as_raw_fd(), operation, fd, &event) } < 0 {
                        registered = false;
                    } else {
                        *previous = Some(index);
                    }
                }
                fd
            } else {
                -1
            };
            self.fds.push(PollFd {
                fd,
                events: 1,
                revents: 0,
            });
        }
        self.failed = false;
        if self.fds.is_empty() {
            return;
        }
        if registered {
            self.events.resize(self.fds.len(), Event::default());
            // The gateway owns every fd until after this service pass. Events
            // are level-triggered and sampled with exactly the old zero wait.
            let count = unsafe {
                epoll_wait(
                    self.epoll.as_ref().unwrap().as_raw_fd(),
                    self.events.as_mut_ptr(),
                    self.events.len() as c_int,
                    0,
                )
            };
            if count >= 0 {
                for event in &self.events[..count as usize] {
                    if let Some(fd) = self.fds.get_mut(event.index as usize) {
                        fd.revents = 1;
                    } else {
                        registered = false;
                    }
                }
                if registered {
                    return;
                }
            }
        }
        // Creation, registration or query failure keeps the original snapshot
        // path; an interrupted fallback still permits every session to progress.
        self.failed = unsafe { poll(self.fds.as_mut_ptr(), self.fds.len() as c_ulong, 0) } < 0;
    }

    pub(super) fn readable(&self, index: usize) -> bool {
        self.failed
            || self
                .fds
                .get(index)
                .is_none_or(|fd| fd.fd < 0 || fd.revents != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        os::unix::net::UnixStream,
    };

    fn snapshot(
        readiness: &mut Readiness,
        sockets: &[(RawFd, usize)],
        registrations: &mut [Option<usize>],
    ) {
        let mut iter = registrations.iter_mut();
        readiness.poll(
            sockets
                .iter()
                .map(|&(fd, _)| Some((fd, iter.next().unwrap()))),
        );
        let mut oracle: Vec<_> = sockets
            .iter()
            .map(|&(fd, _)| PollFd {
                fd,
                events: 1,
                revents: 0,
            })
            .collect();
        assert!(unsafe { poll(oracle.as_mut_ptr(), oracle.len() as c_ulong, 0) } >= 0);
        for (i, fd) in oracle.iter().enumerate() {
            assert_eq!(readiness.readable(i), fd.revents != 0);
        }
    }

    #[test]
    fn persistent_snapshot_matches_poll_through_partial_drains_and_compaction() {
        let (mut a, mut b) = UnixStream::pair().unwrap();
        let (mut c, mut d) = UnixStream::pair().unwrap();
        b.set_nonblocking(true).unwrap();
        d.set_nonblocking(true).unwrap();
        let mut readiness = Readiness::default();
        assert!(readiness.epoll.is_some());
        let mut registrations = [None, None];
        let sockets = [(b.as_raw_fd(), 0), (d.as_raw_fd(), 1)];
        snapshot(&mut readiness, &sockets, &mut registrations);
        a.write_all(&[1, 2]).unwrap();
        c.write_all(&[3]).unwrap();
        snapshot(&mut readiness, &sockets, &mut registrations);
        b.read_exact(&mut [0; 1]).unwrap();
        snapshot(&mut readiness, &sockets, &mut registrations);
        b.read_exact(&mut [0; 1]).unwrap();
        snapshot(&mut readiness, &sockets, &mut registrations);
        registrations.swap(0, 1);
        snapshot(
            &mut readiness,
            &[sockets[1], sockets[0]],
            &mut registrations,
        );
        d.read_exact(&mut [0; 1]).unwrap();
        snapshot(&mut readiness, &[sockets[1]], &mut registrations[..1]);
        drop(c);
        snapshot(&mut readiness, &[sockets[1]], &mut registrations[..1]);
        readiness.poll([None].into_iter());
        assert!(readiness.readable(0));
        readiness.poll(std::iter::empty());
        assert!(readiness.readable(0));
    }

    #[test]
    fn new_connection_registers_a_reused_descriptor() {
        unsafe extern "C" {
            fn dup2(old: c_int, new: c_int) -> c_int;
        }
        let (_a, b) = UnixStream::pair().unwrap();
        let (mut c, mut d) = UnixStream::pair().unwrap();
        let mut readiness = Readiness::default();
        let mut registration = [None];
        snapshot(&mut readiness, &[(b.as_raw_fd(), 0)], &mut registration);
        // Replace only a descriptor owned by this test, preserving its number.
        assert_eq!(unsafe { dup2(d.as_raw_fd(), b.as_raw_fd()) }, b.as_raw_fd());
        registration[0] = None;
        c.write_all(&[7]).unwrap();
        snapshot(&mut readiness, &[(b.as_raw_fd(), 0)], &mut registration);
        d.read_exact(&mut [0; 1]).unwrap();
        snapshot(&mut readiness, &[(b.as_raw_fd(), 0)], &mut registration);
        assert!(!readiness.readable(0));
    }

    #[test]
    fn registration_and_creation_failures_use_the_original_poll_snapshot() {
        let (_a, b) = UnixStream::pair().unwrap();
        let mut readiness = Readiness::default();
        let mut bad = None;
        let mut good = None;
        readiness.poll(
            [
                Some((c_int::MAX, &mut bad)),
                Some((b.as_raw_fd(), &mut good)),
            ]
            .into_iter(),
        );
        assert!(readiness.readable(0));
        assert!(!readiness.readable(1));
        readiness.epoll = None;
        readiness.poll([Some((b.as_raw_fd(), &mut good)), None].into_iter());
        assert!(!readiness.readable(0));
        assert!(readiness.readable(1));
        readiness.failed = true;
        assert!(readiness.readable(0));
    }

    #[test]
    fn failed_epoll_query_uses_poll_and_keeps_other_readable_sessions() {
        let (mut a, b) = UnixStream::pair().unwrap();
        let (mut c, d) = UnixStream::pair().unwrap();
        let mut readiness = Readiness::default();
        let mut registrations = [None, None];
        let sockets = [(b.as_raw_fd(), 0), (d.as_raw_fd(), 1)];
        snapshot(&mut readiness, &sockets, &mut registrations);
        // A valid owned descriptor which is not an epoll instance makes the
        // query fail without corrupting or closing any connection descriptor.
        readiness.epoll = Some(std::fs::File::open("/dev/null").unwrap().into());
        a.write_all(&[1]).unwrap();
        c.write_all(&[2]).unwrap();
        snapshot(&mut readiness, &sockets, &mut registrations);
        assert!(readiness.readable(0));
        assert!(readiness.readable(1));
    }

    #[test]
    fn readiness_matches_poll_through_repeated_reorders_and_partial_reads() {
        let pairs: Vec<_> = (0..17).map(|_| UnixStream::pair().unwrap()).collect();
        let (mut writers, mut readers): (Vec<_>, Vec<_>) = pairs.into_iter().unzip();
        for reader in &readers {
            reader.set_nonblocking(true).unwrap();
        }
        let mut registrations = vec![None; readers.len()];
        let mut readiness = Readiness::default();
        let mut state = 7u64;
        for step in 0..500 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let index = (state as usize) % readers.len();
            writers[index].write_all(&[step as u8; 3]).unwrap();
            let other = (index + 5) % readers.len();
            readers.swap(index, other);
            writers.swap(index, other);
            registrations.swap(index, other);
            let sockets: Vec<_> = readers.iter().map(|r| (r.as_raw_fd(), 0)).collect();
            snapshot(&mut readiness, &sockets, &mut registrations);
            let mut byte = [0; 1];
            match readers[other].read(&mut byte) {
                Ok(1) => {}
                result => panic!("written socket must remain readable: {result:?}"),
            }
            snapshot(&mut readiness, &sockets, &mut registrations);
        }
    }
}
