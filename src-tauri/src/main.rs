fn main() {
    #[cfg(target_os = "linux")]
    {
        std::env::set_var("WINIT_UNIX_APP_ID", "dev.kagent.app");
    }
    k_agent_lib::run();
}
