//! Platform routing for the parts of the app that cannot be written once.
//!
//! Every call site asks this module for the algorithm that fits the host OS
//! instead of carrying its own `cfg` blocks.

pub mod archive;
pub mod exec;
pub mod paths;
pub mod process;
pub mod shell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Os {
    Linux,
    Mac,
    Windows,
    Other,
}

pub const fn os() -> Os {
    #[cfg(target_os = "linux")]
    {
        Os::Linux
    }
    #[cfg(target_os = "macos")]
    {
        Os::Mac
    }
    #[cfg(target_os = "windows")]
    {
        Os::Windows
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Os::Other
    }
}

#[allow(dead_code)]
pub fn os_name() -> &'static str {
    match os() {
        Os::Linux => "linux",
        Os::Mac => "macos",
        Os::Windows => "windows",
        Os::Other => std::env::consts::OS,
    }
}
