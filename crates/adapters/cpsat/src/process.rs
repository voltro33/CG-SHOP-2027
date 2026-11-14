//! One running `cpsat.py`: a request line in, an answer line out.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::Duration;

use mow_core::PortError;

/// How to start the script, kept so a dead process can be started again.
#[derive(Debug, Clone)]
pub(crate) struct Launch {
    pub python: PathBuf,
    pub script: PathBuf,
}

/// The child process, its input, and its output lines as a thread reads
/// them, so an answer can be waited for with a timeout.
pub(crate) struct Worker {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<std::io::Result<String>>,
}

impl Launch {
    pub fn spawn(&self) -> Result<Worker, PortError> {
        // stderr goes to ours: a Python traceback is the best clue there is
        let mut child = Command::new(&self.python)
            .arg(&self.script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| external(&self.python, format!("cannot start python: {e}")))?;

        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        match (stdin, stdout) {
            (Some(stdin), Some(stdout)) => {
                // ends on EOF, when the child dies or is killed
                let (send, lines) = channel();
                std::thread::spawn(move || {
                    for line in BufReader::new(stdout).lines() {
                        if send.send(line).is_err() {
                            break;
                        }
                    }
                });

                Ok(Worker { child, stdin, lines })
            }
            _ => {
                let _ = child.kill();
                Err(external(&self.python, "the child has no pipes".to_owned()))
            }
        }
    }
}

impl Worker {
    /// Sends one request line and waits up to `timeout` for one answer line.
    /// An `Err` means the process is unusable (died, closed its pipe, or is
    /// stuck: `TimedOut`), not that the answer is bad.
    pub fn ask(&mut self, request: &str, timeout: Duration) -> std::io::Result<String> {
        self.stdin.write_all(request.as_bytes())?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;

        match self.lines.recv_timeout(timeout) {
            Ok(line) => line,
            Err(RecvTimeoutError::Timeout) => Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("no answer within {:.0} s", timeout.as_secs_f64()),
            )),
            Err(RecvTimeoutError::Disconnected) => Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "the solver closed its output",
            )),
        }
    }

    /// Lets a test see the restart.
    #[cfg(test)]
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        // the script would end on EOF too; kill so a stuck one cannot hang us
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub(crate) fn external(path: &Path, message: String) -> PortError {
    PortError::External {
        context: path.display().to_string(),
        message,
    }
}
