// PTY-based terminal pane for :terminal and :! commands

#[cfg(unix)]
mod inner {
    use std::io;
    use std::os::fd::RawFd;

    pub struct TerminalPane {
        master_fd: RawFd,
        child_pid: libc::pid_t,
        pub screen_buf: Vec<Vec<char>>,
        pub width: usize,
        pub height: usize,
        cursor_row: usize,
        cursor_col: usize,
    }

    impl TerminalPane {
        pub fn spawn(shell: &str, width: usize, height: usize) -> io::Result<Self> {
            let mut master_fd: RawFd = 0;
            let mut slave_fd: RawFd = 0;
            let ret = unsafe { libc::openpty(&mut master_fd, &mut slave_fd, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) };
            if ret != 0 { return Err(io::Error::last_os_error()); }
            let pid = unsafe { libc::fork() };
            if pid < 0 { return Err(io::Error::last_os_error()); }
            if pid == 0 { // child
                unsafe {
                    libc::close(master_fd);
                    libc::setsid();
                    libc::dup2(slave_fd, 0);
                    libc::dup2(slave_fd, 1);
                    libc::dup2(slave_fd, 2);
                    if slave_fd > 2 { libc::close(slave_fd); }
                    let shell_c = std::ffi::CString::new(shell).unwrap();
                    libc::execvp(shell_c.as_ptr(), [shell_c.as_ptr(), std::ptr::null()].as_ptr());
                    libc::_exit(1);
                }
            }
            // parent
            unsafe {
                libc::close(slave_fd);
                let flags = libc::fcntl(master_fd, libc::F_GETFL);
                libc::fcntl(master_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
            }
            let screen_buf = vec![vec![' '; width]; height];
            Ok(Self { master_fd, child_pid: pid, screen_buf, width, height, cursor_row: 0, cursor_col: 0 })
        }

        pub fn write_input(&self, data: &[u8]) -> io::Result<()> {
            unsafe { libc::write(self.master_fd, data.as_ptr() as *const libc::c_void, data.len()); }
            Ok(())
        }

        pub fn read_output(&mut self) {
            let mut buf = [0u8; 4096];
            loop {
                let n = unsafe { libc::read(self.master_fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
                if n <= 0 { break; }
                for &b in &buf[..n as usize] {
                    let ch = b as char;
                    match ch {
                        '\n' => {
                            self.cursor_row += 1;
                            self.cursor_col = 0;
                            if self.cursor_row >= self.height {
                                self.screen_buf.remove(0);
                                self.screen_buf.push(vec![' '; self.width]);
                                self.cursor_row = self.height - 1;
                            }
                        }
                        '\r' => { self.cursor_col = 0; }
                        '\x08' => { self.cursor_col = self.cursor_col.saturating_sub(1); }
                        c if c >= ' ' => {
                            if self.cursor_col < self.width && self.cursor_row < self.height {
                                self.screen_buf[self.cursor_row][self.cursor_col] = c;
                                self.cursor_col += 1;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        pub fn is_alive(&self) -> bool {
            let mut status: libc::c_int = 0;
            let ret = unsafe { libc::waitpid(self.child_pid, &mut status, libc::WNOHANG) };
            ret == 0 // 0 means still running
        }

        pub fn kill(&self) {
            unsafe { libc::kill(self.child_pid, libc::SIGTERM); }
        }
    }

    impl Drop for TerminalPane {
        fn drop(&mut self) {
            self.kill();
            unsafe { libc::close(self.master_fd); }
        }
    }
}

#[cfg(unix)]
pub use inner::TerminalPane;

#[cfg(not(unix))]
pub struct TerminalPane;

#[cfg(not(unix))]
impl TerminalPane {
    pub fn spawn(_shell: &str, _width: usize, _height: usize) -> std::io::Result<Self> {
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "not supported on this platform"))
    }
}

/// run external command, capture output
pub fn run_command(cmd: &str) -> std::io::Result<String> {
    let output = std::process::Command::new("sh").arg("-c").arg(cmd).output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    Ok(if stderr.is_empty() { stdout } else { format!("{}\n{}", stdout, stderr) })
}

/// filter lines through external command
pub fn filter_through_command(input: &str, cmd: &str) -> std::io::Result<String> {
    use std::io::Write;
    let mut child = std::process::Command::new("sh")
        .arg("-c").arg(cmd)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    if let Some(ref mut stdin) = child.stdin { stdin.write_all(input.as_bytes())?; }
    let output = child.wait_with_output()?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
