use crate::sdk::tasks::CancellationToken;
use std::{
    ffi::{OsStr, OsString},
    io::{self, Read, Write},
    path::PathBuf,
    process::{Child, ExitStatus, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct ProcessSpec {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub env: Vec<(OsString, OsString)>,
    pub clear_env: bool,
    pub cwd: Option<PathBuf>,
    pub stdin: Option<Vec<u8>>,
    pub timeout: Duration,
    pub max_output_bytes: usize,
}

impl ProcessSpec {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            env: Vec::new(),
            clear_env: false,
            cwd: None,
            stdin: None,
            timeout: Duration::from_secs(30),
            max_output_bytes: 1024 * 1024,
        }
    }

    pub fn arg(mut self, value: impl Into<OsString>) -> Self {
        self.args.push(value.into());
        self
    }

    pub fn args<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.args.extend(values.into_iter().map(Into::into));
        self
    }

    pub fn env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    pub fn current_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.cwd = Some(path.into());
        self
    }

    pub fn stdin(mut self, value: impl Into<Vec<u8>>) -> Self {
        self.stdin = Some(value.into());
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn max_output_bytes(mut self, max: usize) -> Self {
        self.max_output_bytes = max;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stream {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug)]
pub struct ProcessOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

impl ProcessOutput {
    pub fn success(&self) -> bool {
        self.status.success()
    }
    pub fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }
    pub fn stderr_text(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProcessError {
    Spawn(String),
    Wait(String),
    Io(String),
    TimedOut(Duration),
    Cancelled,
}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(error) => write!(f, "unable to start process: {error}"),
            Self::Wait(error) => write!(f, "unable to wait for process: {error}"),
            Self::Io(error) => write!(f, "process I/O failed: {error}"),
            Self::TimedOut(timeout) => {
                write!(f, "process timed out after {} seconds", timeout.as_secs())
            }
            Self::Cancelled => f.write_str("process was cancelled"),
        }
    }
}

impl std::error::Error for ProcessError {}

#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessExecutor;

enum Message {
    Chunk(Stream, Vec<u8>),
    Done,
}

impl ProcessExecutor {
    pub fn run(&self, spec: ProcessSpec) -> Result<ProcessOutput, ProcessError> {
        self.run_with(spec, &CancellationToken::default(), |_, _| {})
    }

    /// Runs a process while delivering bounded output chunks to a callback.
    /// The callback runs on the caller thread; readers continue draining both
    /// pipes, retaining at most `max_output_bytes` per stream.
    pub fn run_with(
        &self,
        spec: ProcessSpec,
        cancellation: &CancellationToken,
        mut on_output: impl FnMut(Stream, &[u8]),
    ) -> Result<ProcessOutput, ProcessError> {
        let mut command = crate::core::external::process::command(&spec.program);
        command.args(&spec.args);
        if spec.clear_env {
            command.env_clear();
        }
        for (key, value) in spec.env {
            command.env(key, value);
        }
        if let Some(cwd) = &spec.cwd {
            command.current_dir(cwd);
        }
        command
            .stdin(if spec.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|error| ProcessError::Spawn(error.to_string()))?;
        if let Some(input) = spec.stdin {
            write_stdin(&mut child, input)?;
        }
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ProcessError::Io("stdout pipe unavailable".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| ProcessError::Io("stderr pipe unavailable".into()))?;
        // A bounded channel keeps streaming callbacks from accumulating an
        // unbounded queue when a child writes faster than the host consumes.
        let (sender, receiver) = mpsc::sync_channel(64);
        let out_sender = sender.clone();
        let out_thread = thread::spawn(move || drain(stdout, Stream::Stdout, out_sender));
        let err_thread = thread::spawn(move || drain(stderr, Stream::Stderr, sender));

        let start = Instant::now();
        let mut status = None;
        let mut completed_readers = 0;
        let mut output = ProcessOutputBuilder::new(spec.max_output_bytes);
        let mut failure = None;
        while status.is_none() || completed_readers < 2 {
            if cancellation.is_cancelled() {
                let _ = terminate(&mut child);
                failure = Some(ProcessError::Cancelled);
                break;
            }
            if start.elapsed() >= spec.timeout {
                let _ = terminate(&mut child);
                failure = Some(ProcessError::TimedOut(spec.timeout));
                break;
            }
            match receiver.recv_timeout(Duration::from_millis(15)) {
                Ok(Message::Chunk(stream, bytes)) => {
                    on_output(stream, &bytes);
                    output.push(stream, &bytes);
                }
                Ok(Message::Done) => completed_readers += 1,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => completed_readers = 2,
            }
            if status.is_none() {
                match child.try_wait() {
                    Ok(current) => status = current,
                    Err(error) => {
                        let _ = terminate(&mut child);
                        failure = Some(ProcessError::Wait(error.to_string()));
                        break;
                    }
                }
            }
        }
        if status.is_none() {
            status = child.wait().ok();
        }
        while failure.is_some() && completed_readers < 2 {
            match receiver.recv_timeout(Duration::from_millis(50)) {
                Ok(Message::Chunk(stream, bytes)) => {
                    on_output(stream, &bytes);
                    output.push(stream, &bytes);
                }
                Ok(Message::Done) => completed_readers += 1,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => completed_readers = 2,
            }
        }
        let _ = out_thread.join();
        let _ = err_thread.join();
        while let Ok(message) = receiver.try_recv() {
            if let Message::Chunk(stream, bytes) = message {
                on_output(stream, &bytes);
                output.push(stream, &bytes);
            }
        }
        if let Some(error) = failure {
            return Err(error);
        }
        let status = status.ok_or_else(|| ProcessError::Wait("child status unavailable".into()))?;
        Ok(output.finish(status))
    }
}

fn write_stdin(child: &mut Child, bytes: Vec<u8>) -> Result<(), ProcessError> {
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| ProcessError::Io("stdin pipe unavailable".into()))?;
    thread::Builder::new()
        .name("peninsula-process-stdin".into())
        .spawn(move || {
            let _ = stdin.write_all(&bytes);
        })
        .map_err(|error| ProcessError::Io(error.to_string()))?;
    Ok(())
}

fn drain(
    mut reader: impl Read + Send + 'static,
    stream: Stream,
    sender: mpsc::SyncSender<Message>,
) {
    let mut buffer = [0_u8; 8192];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(count) => {
                if sender
                    .send(Message::Chunk(stream, buffer[..count].to_vec()))
                    .is_err()
                {
                    break;
                }
            }
        }
    }
    let _ = sender.send(Message::Done);
}

fn terminate(child: &mut Child) -> io::Result<()> {
    child.kill()?;
    child.wait().map(|_| ())
}

struct ProcessOutputBuilder {
    max: usize,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stdout_truncated: bool,
    stderr_truncated: bool,
}

impl ProcessOutputBuilder {
    fn new(max: usize) -> Self {
        Self {
            max,
            stdout: Vec::new(),
            stderr: Vec::new(),
            stdout_truncated: false,
            stderr_truncated: false,
        }
    }
    fn push(&mut self, stream: Stream, bytes: &[u8]) {
        let (buffer, truncated) = match stream {
            Stream::Stdout => (&mut self.stdout, &mut self.stdout_truncated),
            Stream::Stderr => (&mut self.stderr, &mut self.stderr_truncated),
        };
        let keep = bytes.len().min(self.max.saturating_sub(buffer.len()));
        buffer.extend_from_slice(&bytes[..keep]);
        *truncated |= keep != bytes.len();
    }
    fn finish(self, status: ExitStatus) -> ProcessOutput {
        ProcessOutput {
            status,
            stdout: self.stdout,
            stderr: self.stderr,
            stdout_truncated: self.stdout_truncated,
            stderr_truncated: self.stderr_truncated,
        }
    }
}

pub fn program_name(program: impl AsRef<OsStr>) -> PathBuf {
    program.as_ref().into()
}

#[cfg(test)]
mod tests {
    use super::{ProcessError, ProcessExecutor, ProcessSpec, Stream};
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    #[cfg(unix)]
    fn shell(script: &str) -> ProcessSpec {
        ProcessSpec::new("/bin/sh")
            .args(["-c", script])
            .timeout(Duration::from_secs(3))
    }

    #[cfg(unix)]
    #[test]
    fn captures_both_streams_with_a_per_stream_limit_and_stream_callback() {
        let chunks = Arc::new(Mutex::new(Vec::new()));
        let observed = chunks.clone();
        let output = ProcessExecutor
            .run_with(
                shell("printf 'hello'; printf 'problem' >&2"),
                &Default::default(),
                move |stream, bytes| observed.lock().unwrap().push((stream, bytes.to_vec())),
            )
            .unwrap();
        assert_eq!(output.stdout_text(), "hello");
        assert_eq!(output.stderr_text(), "problem");
        assert!(output.success());
        assert_eq!(chunks.lock().unwrap().len(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn output_is_capped_without_blocking_pipe_drain() {
        let output = ProcessExecutor
            .run(shell("printf 'abcdefghijklmnop'").max_output_bytes(5))
            .unwrap();
        assert_eq!(output.stdout, b"abcde");
        assert!(output.stdout_truncated);
    }

    #[cfg(unix)]
    #[test]
    fn timeout_and_cancellation_terminate_the_child() {
        let timed = ProcessExecutor.run(shell("sleep 2").timeout(Duration::from_millis(20)));
        assert!(matches!(timed, Err(ProcessError::TimedOut(_))));
        let token = crate::sdk::tasks::CancellationToken::default();
        token.cancel();
        let cancelled = ProcessExecutor.run_with(shell("sleep 2"), &token, |_, _| {});
        assert!(matches!(cancelled, Err(ProcessError::Cancelled)));
    }

    #[test]
    fn stream_variants_are_stable() {
        assert_ne!(Stream::Stdout, Stream::Stderr);
    }
}
