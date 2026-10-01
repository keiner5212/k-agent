fn main() {
    #[cfg(target_os = "linux")]
    {
        std::env::set_var("WINIT_UNIX_APP_ID", "dev.kagent.app");
    }
    #[cfg(unix)]
    detach_from_terminal();
    k_agent_lib::run();
}

// Shell returns immediately. The child keeps the window.
// K_AGENT_FOREGROUND=1 stays attached for logs.
#[cfg(unix)]
fn detach_from_terminal() {
    if std::env::var_os("K_AGENT_FOREGROUND").is_some() {
        return;
    }
    unsafe {
        if libc::isatty(libc::STDOUT_FILENO) == 0 {
            return;
        }
        let pid = libc::fork();
        if pid < 0 {
            return;
        }
        if pid > 0 {
            std::process::exit(0);
        }
        let _ = libc::setsid();
        let pid = libc::fork();
        if pid < 0 {
            return;
        }
        if pid > 0 {
            std::process::exit(0);
        }
        let null = libc::open(c"/dev/null".as_ptr(), libc::O_RDWR);
        if null >= 0 {
            libc::dup2(null, libc::STDIN_FILENO);
            libc::dup2(null, libc::STDOUT_FILENO);
            libc::dup2(null, libc::STDERR_FILENO);
            if null > 2 {
                libc::close(null);
            }
        }
    }
}
