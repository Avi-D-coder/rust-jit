//! Safe nonblocking pipe IO: inherited pipes cannot defeat the helper deadline.
use super::process_guard::{ProcessControl, StopReason};
use std::{
    io::{self, Read, Write},
    os::fd::AsFd,
    thread,
    time::{Duration, Instant},
};

pub(super) struct Interruptible<T> {
    inner: T,
    control: ProcessControl,
    terminal_drain_deadline: Option<Instant>,
}
impl<T: AsFd> Interruptible<T> {
    pub(super) fn new(inner: T, control: ProcessControl) -> io::Result<Self> {
        let flags = rustix::fs::fcntl_getfl(&inner)?;
        rustix::fs::fcntl_setfl(&inner, flags | rustix::fs::OFlags::NONBLOCK)?;
        Ok(Self {
            inner,
            control,
            terminal_drain_deadline: None,
        })
    }
    fn retry(&mut self) -> io::Result<()> {
        if let Some((reason, _)) = self.control.status()? {
            // Concurrent fork/exec can briefly retain a close-on-exec pipe.
            // Allow actual EOF to arrive after normal exit within one fixed
            // drain window, never resetting it for subsequent reads/writes.
            let now = Instant::now();
            let expires = self
                .terminal_drain_deadline
                .get_or_insert(now + Duration::from_millis(100));
            if reason == StopReason::Exited && now < *expires {
                thread::sleep(Duration::from_millis(1));
                return Ok(());
            }
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "macro helper terminated while pipe is held open",
            ));
        }
        thread::sleep(Duration::from_millis(1));
        Ok(())
    }
}
impl<T: Read + AsFd> Read for Interruptible<T> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        loop {
            match self.inner.read(bytes) {
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    if let Err(terminal) = self.retry() {
                        // Exit can close the pipe between EAGAIN and the child
                        // status check. Observe actual queued bytes/EOF once;
                        // a still-open inherited pipe remains a typed failure.
                        return match self.inner.read(bytes) {
                            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                                Err(terminal)
                            }
                            result => result,
                        };
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                result => return result,
            }
        }
    }
}
impl<T: Write + AsFd> Write for Interruptible<T> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        loop {
            match self.inner.write(bytes) {
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => self.retry()?,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                result => return result,
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proc_client::process_guard::{ProcessGuard, StopReason};
    use std::{
        os::unix::net::UnixStream,
        process::{Command, Stdio},
        time::Instant,
    };
    fn guard() -> ProcessGuard {
        let mut command = Command::new("/bin/sleep");
        command
            .arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        ProcessGuard::spawn(&mut command, Duration::from_millis(60)).unwrap()
    }
    #[test]
    fn normal_exit_between_would_block_and_status_preserves_actual_pipe_eof() {
        struct RaceReader {
            stream: UnixStream,
            pending_reads: u8,
        }
        impl AsFd for RaceReader {
            fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> {
                self.stream.as_fd()
            }
        }
        impl Read for RaceReader {
            fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
                if self.pending_reads > 0 {
                    self.pending_reads -= 1;
                    return Err(io::ErrorKind::WouldBlock.into());
                }
                self.stream.read(bytes)
            }
        }
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "exit 0"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let guard = ProcessGuard::spawn(&mut command, Duration::from_secs(3)).unwrap();
        let (reason, status) = guard.wait_armed().unwrap();
        assert_eq!(reason, StopReason::Exited);
        assert!(status.success());
        let (stream, peer) = UnixStream::pair().unwrap();
        drop(peer);
        let mut reader = Interruptible::new(
            RaceReader {
                stream,
                pending_reads: 3,
            },
            guard.control(),
        )
        .unwrap();
        assert_eq!(reader.read(&mut [0; 1]).unwrap(), 0);
        let (stream, _holder) = UnixStream::pair().unwrap();
        let mut reader = Interruptible::new(stream, guard.control()).unwrap();
        let started = Instant::now();
        assert_eq!(
            reader.read(&mut [0; 1]).unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
        assert!(started.elapsed() < Duration::from_secs(5));
        let deadline = reader.terminal_drain_deadline;
        assert_eq!(
            reader.read(&mut [0; 1]).unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
        assert_eq!(
            reader.terminal_drain_deadline, deadline,
            "drain window must not restart"
        );
    }
    #[test]
    fn inherited_reader_remains_bounded_after_owned_helper_deadline() {
        let mut guard = guard();
        let (pipe, _external_holder) = UnixStream::pair().unwrap();
        let mut reader = Interruptible::new(pipe, guard.control()).unwrap();
        let started = Instant::now();
        assert_eq!(
            reader.read(&mut [0; 1]).unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
        assert_eq!(guard.cancel().unwrap().0, StopReason::Deadline);
        assert!(started.elapsed() < Duration::from_secs(5));
    }
    #[test]
    fn inherited_writer_remains_bounded_when_peer_never_reads() {
        let mut guard = guard();
        let (pipe, _external_holder) = UnixStream::pair().unwrap();
        let mut writer = Interruptible::new(pipe, guard.control()).unwrap();
        let started = Instant::now();
        assert_eq!(
            writer
                .write_all(&vec![0; 8 * 1024 * 1024])
                .unwrap_err()
                .kind(),
            io::ErrorKind::BrokenPipe
        );
        assert_eq!(guard.cancel().unwrap().0, StopReason::Deadline);
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
