//! Bounded startup/version probes before any procedural library loads.
use super::{
    process_guard::{ProcessGuard, StopReason},
    transport::BoundedReader,
};
use std::{
    io::{self, BufReader, Read, Write},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::Duration,
};

pub(super) struct Output {
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
    pub(super) status: ExitStatus,
}

pub(super) fn run(
    command: &mut Command,
    input: &[u8],
    limit: usize,
    timeout: Duration,
) -> io::Result<Output> {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut guard = ProcessGuard::spawn(command, timeout)?;
    let mut stdin = super::interruptible::Interruptible::new(guard.take_stdin()?, guard.control())?;
    let stdout = super::interruptible::Interruptible::new(guard.take_stdout()?, guard.control())?;
    let stderr = super::interruptible::Interruptible::new(guard.take_stderr()?, guard.control())?;
    let control = guard.control();
    let errors = thread::Builder::new()
        .name("lrr-macro-probe-stderr".into())
        .spawn(move || {
            let mut reader = BoundedReader::new(BufReader::new(stderr), limit, 0)?;
            let mut bytes = Vec::new();
            let result = reader.read_to_end(&mut bytes);
            if result.is_err() {
                let _ = control.terminate();
            }
            result.map(|_| bytes)
        })?;
    let result: io::Result<Vec<u8>> = (|| {
        stdin.write_all(input)?;
        drop(stdin);
        let mut reader = BoundedReader::new(BufReader::new(stdout), limit, 0)?;
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        Ok(bytes)
    })();
    if result.is_err() {
        let _ = guard.cancel();
    }
    let terminal = guard.wait_armed();
    let errors = errors
        .join()
        .map_err(|_| io::Error::other("macro stderr reader panicked"))?;
    let (reason, status) = terminal?;
    if reason == StopReason::Deadline {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "macro startup deadline exceeded",
        ));
    }
    Ok(Output {
        stdout: result?,
        stderr: errors?,
        status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_channels_remain_separate_and_preserve_actual_exit_status() {
        let output = run(
            Command::new("/bin/sh").args(["-c", "cat; printf diagnostic >&2; exit 7"]),
            b"protocol",
            64,
            Duration::from_secs(3),
        )
        .unwrap();
        assert_eq!(output.stdout, b"protocol");
        assert_eq!(output.stderr, b"diagnostic");
        assert_eq!(output.status.code(), Some(7));
    }

    #[test]
    fn startup_hang_has_deadline_and_no_successful_empty_reply() {
        let error = run(
            Command::new("/bin/sh").args(["-c", "exec /bin/sleep 30"]),
            b"",
            64,
            Duration::from_millis(40),
        )
        .err()
        .unwrap();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    }

    #[test]
    fn oversized_startup_stdout_or_stderr_fails_and_reaps_child() {
        for source in ["printf 123456789", "printf 123456789 >&2"] {
            let error = run(
                Command::new("/bin/sh").args(["-c", source]),
                b"",
                4,
                Duration::from_secs(3),
            )
            .err()
            .unwrap();
            assert!(error.to_string().contains("allowance exhausted"));
        }
    }
}
