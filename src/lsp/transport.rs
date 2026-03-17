// JSON-RPC transport over stdio (dedicated reader thread + channel)

use std::io::{self, BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

pub struct Transport {
    child: Child,
    rx: Receiver<serde_json::Value>,
    stdin: std::process::ChildStdin,
}

impl Transport {
    pub fn spawn(command: &str, args: &[&str]) -> io::Result<Self> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin = child.stdin.take().ok_or(io::Error::new(io::ErrorKind::Other, "no stdin"))?;
        let stdout = child.stdout.take().ok_or(io::Error::new(io::ErrorKind::Other, "no stdout"))?;
        let (tx, rx) = mpsc::channel();
        // spawn reader thread
        thread::spawn(move || { read_loop(stdout, tx); });
        Ok(Self { child, rx, stdin })
    }

    pub fn send(&mut self, msg: &serde_json::Value) -> io::Result<()> {
        let body = serde_json::to_string(msg)?;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        self.stdin.write_all(header.as_bytes())?;
        self.stdin.write_all(body.as_bytes())?;
        self.stdin.flush()
    }

    pub fn try_recv(&mut self) -> Option<serde_json::Value> {
        self.rx.try_recv().ok()
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
    }
}

fn read_loop(stdout: std::process::ChildStdout, tx: Sender<serde_json::Value>) {
    let mut reader = BufReader::new(stdout);
    loop {
        // read headers
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => return, // eof
                Ok(_) => {
                    let line = line.trim();
                    if line.is_empty() { break; } // end of headers
                    if let Some(len_str) = line.strip_prefix("Content-Length: ") {
                        content_length = len_str.trim().parse().ok();
                    }
                }
                Err(_) => return,
            }
        }
        // read body
        if let Some(len) = content_length {
            let mut buf = vec![0u8; len];
            if io::Read::read_exact(&mut reader, &mut buf).is_err() { return; }
            if let Ok(msg) = serde_json::from_slice(&buf) {
                if tx.send(msg).is_err() { return; }
            }
        }
    }
}
