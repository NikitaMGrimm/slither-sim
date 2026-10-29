//! Linux readiness snapshot: one nonblocking query replaces empty receives.
//! It never waits: the delivery clock and channel still control gateway wakeups.
use std::{
    ffi::{c_int, c_short, c_ulong},
    os::fd::RawFd,
};

#[repr(C)]
struct PollFd {
    fd: c_int,
    events: c_short,
    revents: c_short,
}

unsafe extern "C" {
    fn poll(fds: *mut PollFd, count: c_ulong, timeout: c_int) -> c_int;
}

#[derive(Default)]
pub(super) struct Readiness {
    fds: Vec<PollFd>,
    failed: bool,
}

impl Readiness {
    pub(super) fn poll(&mut self, sockets: impl Iterator<Item = Option<RawFd>>) {
        self.fds.clear();
        self.fds.extend(sockets.map(|fd| PollFd {
            fd: fd.unwrap_or(-1),
            events: 1, // POLLIN; errors and hangups are reported regardless.
            revents: 0,
        }));
        // The gateway owns these descriptors and cannot close them until the
        // following service pass. poll borrows the initialized array only here.
        self.failed = !self.fds.is_empty()
            && unsafe { poll(self.fds.as_mut_ptr(), self.fds.len() as c_ulong, 0) } < 0;
    }

    pub(super) fn readable(&self, index: usize) -> bool {
        // An interrupted query must not suppress progress. Handshakes have no
        // descriptor in this snapshot and retain their existing service path.
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
        os::fd::AsRawFd,
        os::unix::net::UnixStream,
    };

    #[test]
    fn snapshot_tracks_input_drain_hangup_and_missing_descriptors() {
        let (mut client, mut server) = UnixStream::pair().unwrap();
        server.set_nonblocking(true).unwrap();
        let mut readiness = Readiness::default();
        let fd = server.as_raw_fd();
        let refresh = |readiness: &mut Readiness| {
            readiness.poll([Some(fd), None].into_iter());
        };
        refresh(&mut readiness);
        assert!(!readiness.readable(0));
        assert!(readiness.readable(1));
        client.write_all(&[7]).unwrap();
        refresh(&mut readiness);
        assert!(readiness.readable(0));
        assert_eq!(server.read(&mut [0; 1]).unwrap(), 1);
        refresh(&mut readiness);
        assert!(!readiness.readable(0));
        drop(client);
        refresh(&mut readiness);
        assert!(readiness.readable(0));
        assert_eq!(server.read(&mut [0; 1]).unwrap(), 0);
    }

    #[test]
    fn failed_snapshot_and_invalid_descriptors_still_get_serviced() {
        let mut readiness = Readiness::default();
        readiness.poll([Some(c_int::MAX)].into_iter());
        assert!(readiness.readable(0));
        readiness.poll(std::iter::empty());
        assert!(readiness.readable(0));
        let (_client, server) = UnixStream::pair().unwrap();
        readiness.poll([Some(server.as_raw_fd())].into_iter());
        assert!(!readiness.readable(0));
        readiness.failed = true;
        assert!(readiness.readable(0));
    }
}
