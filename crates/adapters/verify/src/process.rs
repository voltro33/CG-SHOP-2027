//! One running `verify.py`: a request line in, an answer line out.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use mow_core::PortError;

/// How to start the script, kept so a dead process can be started again.
#[derive(Debug, Clone)]
pub(crate) struct Launch {
    pub python: PathBuf,
    pub script: PathBuf,
}

/// The child process and both ends of its pipes.
pub(crate) struct Worker {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
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
            (Some(stdin), Some(stdout)) => Ok(Worker {
                child,
                stdin,
                stdout: BufReader::new(stdout),
            }),
            _ => {
                let _ = child.kill();
                Err(external(&self.python, "the child has no pipes".to_owned()))
            }
        }
    }
}

impl Worker {
    /// Sends one request line and reads one answer line. An `Err` means the
    /// process is unusable (died, closed its pipe), not that the answer is bad.
    pub fn ask(&mut self, request: &str) -> std::io::Result<String> {
        self.stdin.write_all(request.as_bytes())?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;

        let mut line = String::new();
        if self.stdout.read_line(&mut line)? == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "the verifier closed its output",
            ));
        }

        Ok(line)
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
