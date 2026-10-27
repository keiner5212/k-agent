use std::process::{Child, Command};

/// Prepare a command so the child cannot outlive the app.
pub fn protect(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                set_parent_death_signal();
                Ok(())
            });
        }
    }
    #[cfg(not(unix))]
    {
        let _ = command;
    }
}

/// Prepare a command that also leaves the app process group.
pub fn protect_detached(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                set_parent_death_signal();
                if setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    #[cfg(not(unix))]
    {
        let _ = command;
    }
}

/// Spawn a child that cannot outlive the app.
///
/// Unix gets a parent death signal. Windows gets the app job object, which
/// kills the whole tree when the app dies.
pub fn spawn_protected(command: &mut Command) -> std::io::Result<Child> {
    protect(command);
    #[cfg(windows)]
    {
        spawn_windows(command)
    }
    #[cfg(not(windows))]
    {
        command.spawn()
    }
}

/// Put an already running process under the app job object.
pub fn protect_pid(pid: u32) -> bool {
    #[cfg(windows)]
    {
        unsafe { protect_pid_windows(pid) }
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
        false
    }
}

/// Kill a process and everything it started.
pub fn kill_tree(pid: u32) {
    #[cfg(unix)]
    {
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
    }
}

/// Kill every process the app started.
pub fn kill_own_process_group() {
    #[cfg(unix)]
    {
        kill_group_members();
    }
    #[cfg(windows)]
    {
        terminate_job();
    }
    #[cfg(not(any(unix, windows)))]
    {}
}

#[cfg(unix)]
pub unsafe fn set_parent_death_signal() {
    libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL as libc::c_ulong);
}

#[cfg(unix)]
extern "C" {
    fn setsid() -> i32;
}

/// Kill the members of the app process group, this process excepted.
#[cfg(unix)]
fn kill_group_members() {
    unsafe {
        let pid = libc::getpid() as i32;
        let group = libc::getpgid(pid);
        if group != pid {
            // The app shares a group with something else. Killing it would hit that too.
            return;
        }
        for child in group_members(group) {
            if child != pid {
                libc::kill(child, libc::SIGKILL);
            }
        }
    }
}

#[cfg(unix)]
fn group_members(group: i32) -> Vec<i32> {
    let mut members = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return members;
    };
    for entry in entries.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let Ok(pid) = name.parse::<i32>() else {
            continue;
        };
        if process_group(pid) == group {
            members.push(pid);
        }
    }
    members
}

#[cfg(unix)]
fn process_group(pid: i32) -> i32 {
    // /proc/<pid>/stat: pid (comm) state ppid pgrp session ...
    let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return -1;
    };
    let Some(name_end) = stat.rfind(')') else {
        return -1;
    };
    let fields: Vec<&str> = stat[name_end + 1..].split_whitespace().collect();
    fields
        .get(2)
        .and_then(|value| value.parse().ok())
        .unwrap_or(-1)
}

#[cfg(windows)]
mod windows_job {
    use std::ffi::c_void;
    use std::mem::size_of;
    use std::sync::OnceLock;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    pub struct Job(HANDLE);

    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}

    static JOB: OnceLock<Option<Job>> = OnceLock::new();

    pub fn job() -> Option<&'static Job> {
        JOB.get_or_init(|| unsafe { create_job() }).as_ref()
    }

    unsafe fn create_job() -> Option<Job> {
        let Ok(handle) = (unsafe { CreateJobObjectW(None, PCWSTR::null()) }) else {
            return None;
        };
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let ok = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const c_void,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if ok.is_err() {
            unsafe {
                let _ = CloseHandle(handle);
            }
            return None;
        }
        Some(Job(handle))
    }

    pub fn assign(process: HANDLE) -> bool {
        let Some(job) = job() else {
            return false;
        };
        unsafe { AssignProcessToJobObject(job.0, process) }.is_ok()
    }

    pub fn assign_pid(pid: u32) -> bool {
        use windows::Win32::System::Threading::{
            OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
        };
        let Ok(process) =
            (unsafe { OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, false, pid) })
        else {
            return false;
        };
        let assigned = assign(process);
        unsafe {
            let _ = CloseHandle(process);
        }
        assigned
    }

    pub fn terminate() {
        if let Some(job) = job() {
            unsafe {
                let _ = TerminateJobObject(job.0, 1);
            }
        }
    }

    pub fn resume_primary_thread(pid: u32) {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
        };
        use windows::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};

        unsafe {
            let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) else {
                return;
            };
            let mut entry = THREADENTRY32 {
                dwSize: size_of::<THREADENTRY32>() as u32,
                ..Default::default()
            };
            if Thread32First(snapshot, &mut entry).is_ok() {
                loop {
                    if entry.th32OwnerProcessID == pid {
                        if let Ok(thread) =
                            OpenThread(THREAD_SUSPEND_RESUME, false, entry.th32ThreadID)
                        {
                            let _ = ResumeThread(thread);
                            let _ = CloseHandle(thread);
                        }
                        break;
                    }
                    if Thread32Next(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snapshot);
        }
    }
}

#[cfg(windows)]
fn spawn_windows(command: &mut Command) -> std::io::Result<Child> {
    use std::os::windows::io::AsRawHandle;
    use std::os::windows::process::CommandExt;

    const CREATE_SUSPENDED: u32 = 0x0000_0004;

    command.creation_flags(CREATE_SUSPENDED);
    let mut child = command.spawn()?;
    unsafe {
        windows_job::assign(windows::Win32::Foundation::HANDLE(child.as_raw_handle()));
    }
    windows_job::resume_primary_thread(child.id());
    Ok(child)
}

#[cfg(windows)]
unsafe fn protect_pid_windows(pid: u32) -> bool {
    windows_job::assign_pid(pid)
}

#[cfg(windows)]
fn terminate_job() {
    windows_job::terminate();
}
