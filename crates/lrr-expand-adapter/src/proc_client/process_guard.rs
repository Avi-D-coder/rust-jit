//! Owned helper lifetime. A deadline kills and reaps the child to unblock IO.
use std::{
    io,
    process::{Child, Command, ExitStatus},
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StopReason {
    Exited,
    Deadline,
    Cancelled,
}

struct State {
    child: Child,
    terminal: Option<(StopReason, ExitStatus)>,
    stopped: bool,
    deadline: Option<Instant>,
    group: Option<rustix::process::Pid>,
}

#[derive(Clone)]
pub(super) struct ProcessControl(Arc<(Mutex<State>, Condvar)>);

impl ProcessControl {
    pub(super) fn status(&self) -> io::Result<Option<(StopReason, ExitStatus)>> {
        let mut state = self.0.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.terminal.is_none()
            && let Some(status) = state.child.try_wait()?
        {
            stop_group(&mut state)?;
            state.terminal = Some((StopReason::Exited, status));
            state.stopped = true;
            self.0.1.notify_all();
        }
        if !state.stopped
            && state
                .deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
        {
            stop(&mut state, StopReason::Deadline)?;
            self.0.1.notify_all();
        }
        Ok(state.terminal)
    }
    pub(super) fn arm(&self, timeout: Duration) -> io::Result<()> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .filter(|_| !timeout.is_zero())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "invalid helper deadline")
            })?;
        let mut state = self.0.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.stopped {
            return Err(io::Error::other("macro helper already terminated"));
        }
        state.deadline = Some(deadline);
        self.0.1.notify_all();
        Ok(())
    }

    pub(super) fn disarm(&self) -> io::Result<()> {
        let mut state = self.0.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.stopped {
            return Err(io::Error::other("macro helper already terminated"));
        }
        if let Some(status) = state.child.try_wait()? {
            stop_group(&mut state)?;
            state.terminal = Some((StopReason::Exited, status));
            state.stopped = true;
            self.0.1.notify_all();
            return Err(io::Error::other(
                "macro helper exited before output admission",
            ));
        }
        if state
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            stop(&mut state, StopReason::Deadline)?;
            self.0.1.notify_all();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "macro request deadline exceeded",
            ));
        }
        state.deadline = None;
        self.0.1.notify_all();
        Ok(())
    }

    pub(super) fn terminate(&self) -> io::Result<(StopReason, ExitStatus)> {
        let result = stop(
            &mut self.0.0.lock().unwrap_or_else(|e| e.into_inner()),
            StopReason::Cancelled,
        );
        self.0.1.notify_all();
        result
    }
}

pub(super) struct ProcessGuard {
    shared: Arc<(Mutex<State>, Condvar)>,
    watcher: Option<JoinHandle<()>>,
}

impl ProcessGuard {
    pub(super) fn id(&self) -> u32 {
        self.shared
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .child
            .id()
    }
    /// This is the only grouped-child constructor: never signal a caller's
    /// arbitrary process group or the coordinator's own group.
    pub(super) fn spawn(command: &mut Command, timeout: Duration) -> io::Result<Self> {
        if timeout.is_zero() || Instant::now().checked_add(timeout).is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid helper deadline",
            ));
        }
        use std::os::unix::process::CommandExt;
        let mut child = command.process_group(0).spawn()?;
        let group = i32::try_from(child.id())
            .ok()
            .and_then(rustix::process::Pid::from_raw)
            .filter(|pid| *pid != rustix::process::Pid::INIT);
        let Some(group) = group else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other("invalid owned helper group"));
        };
        Self::adopt(child, timeout, Some(group))
    }
    pub(super) fn control(&self) -> ProcessControl {
        ProcessControl(self.shared.clone())
    }
    pub(super) fn take_stdin(&mut self) -> io::Result<std::process::ChildStdin> {
        self.shared
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("helper stdin absent"))
    }

    pub(super) fn take_stdout(&mut self) -> io::Result<std::process::ChildStdout> {
        self.shared
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("helper stdout absent"))
    }

    pub(super) fn take_stderr(&mut self) -> io::Result<std::process::ChildStderr> {
        self.shared
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("helper stderr absent"))
    }

    pub(super) fn wait_armed(&self) -> io::Result<(StopReason, ExitStatus)> {
        loop {
            if let Some(terminal) = self.status()? {
                return Ok(terminal);
            }
            if self
                .shared
                .0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .deadline
                .is_none()
            {
                return Err(io::Error::other("cannot wait for an idle macro helper"));
            }
            thread::sleep(Duration::from_millis(1));
        }
    }

    /// Adopt an already-authorized child. No command or filesystem access here.
    #[cfg(test)]
    pub(super) fn new(child: Child, timeout: Duration) -> io::Result<Self> {
        Self::adopt(child, timeout, None)
    }
    fn adopt(
        mut child: Child,
        timeout: Duration,
        group: Option<rustix::process::Pid>,
    ) -> io::Result<Self> {
        let Some(deadline) = Instant::now()
            .checked_add(timeout)
            .filter(|_| !timeout.is_zero())
        else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid helper deadline",
            ));
        };
        let shared = Arc::new((
            Mutex::new(State {
                child,
                terminal: None,
                stopped: false,
                deadline: Some(deadline),
                group,
            }),
            Condvar::new(),
        ));
        let watch = shared.clone();
        let watcher = thread::Builder::new()
            .name("lrr-macro-deadline".into())
            .spawn(move || {
                let (lock, wake) = &*watch;
                let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
                while !state.stopped {
                    let Some(deadline) = state.deadline else {
                        state = wake.wait(state).unwrap_or_else(|e| e.into_inner());
                        continue;
                    };
                    let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                        let _ = stop(&mut state, StopReason::Deadline);
                        break;
                    };
                    state = wake
                        .wait_timeout(state, remaining)
                        .unwrap_or_else(|e| e.into_inner())
                        .0;
                }
            });
        match watcher {
            Ok(watcher) => Ok(Self {
                shared,
                watcher: Some(watcher),
            }),
            Err(error) => {
                let mut state = shared.0.lock().unwrap_or_else(|e| e.into_inner());
                let _ = stop(&mut state, StopReason::Cancelled);
                Err(error)
            }
        }
    }

    pub(super) fn status(&self) -> io::Result<Option<(StopReason, ExitStatus)>> {
        self.control().status()
    }

    pub(super) fn cancel(&mut self) -> io::Result<(StopReason, ExitStatus)> {
        let result = {
            let mut state = self.shared.0.lock().unwrap_or_else(|e| e.into_inner());
            stop(&mut state, StopReason::Cancelled)
        };
        self.shared.1.notify_all();
        if let Some(watcher) = self.watcher.take() {
            watcher
                .join()
                .map_err(|_| io::Error::other("macro deadline watcher panicked"))?;
        }
        result
    }
}

fn stop(state: &mut State, reason: StopReason) -> io::Result<(StopReason, ExitStatus)> {
    if let Some(terminal) = state.terminal {
        stop_group(state)?;
        return Ok(terminal);
    }
    let exited = state.child.try_wait()?;
    stop_group(state)?;
    let terminal = if let Some(status) = exited {
        (StopReason::Exited, status)
    } else {
        // A concurrent exit can make kill fail; wait still determines the
        // actual terminal status and reaps the owned child.
        let _ = state.child.kill();
        (reason, state.child.wait()?)
    };
    state.terminal = Some(terminal);
    state.stopped = true;
    Ok(terminal)
}

fn stop_group(state: &mut State) -> io::Result<()> {
    if let Some(group) = state.group {
        match rustix::process::kill_process_group(group, rustix::process::Signal::KILL) {
            Ok(()) | Err(rustix::io::Errno::SRCH) => {
                state.group = None;
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        let _ = self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Read,
        process::{Command, Stdio},
    };

    fn sleeping_child() -> Child {
        Command::new("/bin/sh")
            .args(["-c", "exec /bin/sleep 30"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
    }

    #[test]
    fn deadline_terminates_and_reaps_actual_helper_and_unblocks_read() {
        let mut child = sleeping_child();
        let mut stdout = child.stdout.take().unwrap();
        let started = Instant::now();
        let mut guard = ProcessGuard::new(child, Duration::from_millis(80)).unwrap();
        assert_eq!(stdout.read(&mut [0; 1]).unwrap(), 0);
        let (reason, status) = guard.cancel().unwrap();
        assert_eq!(reason, StopReason::Deadline);
        assert!(!status.success());
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(guard.status().unwrap(), Some((reason, status)));
    }

    #[test]
    fn cancellation_reaps_once_without_waiting_for_deadline() {
        let child = sleeping_child();
        let started = Instant::now();
        let mut guard = ProcessGuard::new(child, Duration::from_secs(30)).unwrap();
        let terminal = guard.cancel().unwrap();
        assert_eq!(terminal.0, StopReason::Cancelled);
        assert!(!terminal.1.success());
        assert_eq!(guard.cancel().unwrap(), terminal);
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn actual_normal_exit_keeps_status_and_cancels_watcher() {
        let mut child = Command::new("/bin/sh")
            .args(["-c", "exit 7"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let mut guard = ProcessGuard::new(child, Duration::from_secs(30)).unwrap();
        stdout.read_to_end(&mut Vec::new()).unwrap();
        // EOF only establishes that the pipe closed. Observe the actual
        // process status before exercising cancellation of an exited helper.
        let until = Instant::now() + Duration::from_secs(5);
        while guard.status().unwrap().is_none() {
            assert!(Instant::now() < until, "helper exit status did not arrive");
            thread::sleep(Duration::from_millis(1));
        }
        let terminal = guard.cancel().unwrap();
        assert_eq!(terminal.0, StopReason::Exited);
        assert_eq!(terminal.1.code(), Some(7));
        assert_eq!(guard.status().unwrap(), Some(terminal));
    }

    #[test]
    fn invalid_deadline_reaps_adopted_child() {
        let mut child = sleeping_child();
        let mut stdout = child.stdout.take().unwrap();
        assert!(ProcessGuard::new(child, Duration::ZERO).is_err());
        assert_eq!(stdout.read(&mut [0; 1]).unwrap(), 0);
    }

    #[test]
    fn idle_session_can_rearm_deadline_and_cannot_reuse_terminated_helper() {
        let mut child = sleeping_child();
        let mut stdout = child.stdout.take().unwrap();
        let mut guard = ProcessGuard::new(child, Duration::from_secs(1)).unwrap();
        let control = guard.control();
        control.disarm().unwrap();
        assert!(guard.wait_armed().is_err());
        control.arm(Duration::from_millis(40)).unwrap();
        assert_eq!(stdout.read(&mut [0; 1]).unwrap(), 0);
        assert_eq!(guard.wait_armed().unwrap().0, StopReason::Deadline);
        assert!(control.arm(Duration::from_secs(1)).is_err());
        assert!(control.disarm().is_err());
        assert_eq!(guard.cancel().unwrap().0, StopReason::Deadline);
    }

    #[test]
    fn standard_child_kill_leaves_inherited_pipe_blocked_until_owned_group_cleanup() {
        use std::{io::BufRead, os::unix::process::CommandExt, sync::mpsc};
        let mut child = Command::new("/bin/sh")
            .args(["-c", "/bin/sleep 30 & printf '%s\\n' $!; wait"])
            .process_group(0)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let group = rustix::process::Pid::from_raw(i32::try_from(child.id()).unwrap()).unwrap();
        assert_ne!(group, rustix::process::Pid::INIT);
        let mut stdout = std::io::BufReader::new(child.stdout.take().unwrap());
        let mut descendant = String::new();
        stdout.read_line(&mut descendant).unwrap();
        assert!(descendant.trim().parse::<u32>().unwrap() > 1);
        child.kill().unwrap();
        let status = child.wait().unwrap();
        assert!(!status.success());
        let (send, receive) = mpsc::channel();
        let reader = thread::spawn(move || {
            let mut bytes = vec![];
            send.send(stdout.read_to_end(&mut bytes)).unwrap();
        });
        let before = receive.recv_timeout(Duration::from_millis(80));
        // Clean up before checking the witness, so a failed assertion cannot
        // leave the controlled descendant or reader thread behind.
        rustix::process::kill_process_group(group, rustix::process::Signal::KILL).unwrap();
        assert!(
            matches!(before, Err(mpsc::RecvTimeoutError::Timeout)),
            "stdlib parent kill unexpectedly closed the inherited pipe"
        );
        receive
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap();
        reader.join().unwrap();
    }
}
