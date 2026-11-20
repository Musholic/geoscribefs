use geoscribefs::server::ServerConfig;
use nix::{
    sys::signal::{Signal, kill},
    unistd::Pid,
};
use tokio::process::{Child, Command};

use crate::common::wait_for_server;

pub async fn start_server_process(config: ServerConfig) -> ServerHandle {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_geoscribefs"));
    cmd.arg("--addr")
        .arg(&config.addr)
        .arg("--token")
        .arg(&config.token)
        .arg("start");

    for peer in &config.peers {
        cmd.arg("--peers").arg(peer);
    }

    for path in &config.volume_paths {
        cmd.arg("--volume-paths").arg(path);
    }

    let child = cmd
        .spawn()
        .expect("Failed to spawn geoscribefs server process");
    wait_for_server(&config.addr).await;

    ServerHandle { child: Some(child) }
}

/// Gracefully exit the server process (with SIGTERM) on drop
pub struct ServerHandle {
    child: Option<Child>,
}

impl ServerHandle {
    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().and_then(Child::id)
    }

    fn send_sigterm(&self) {
        if let Some(pid) = self.pid() {
            tracing::info!(pid, "Sending SIGTERM to process");
            if let Err(e) = kill(Pid::from_raw(pid as libc::pid_t), Signal::SIGTERM) {
                tracing::warn!(pid, error = ?e, "Failed to send SIGTERM");
            }
        }
    }

    /// Wait for the process to exit after sending SIGTERM
    pub async fn shutdown(&mut self) {
        self.send_sigterm();
        if let Some(child) = self.child.as_mut() {
            child
                .wait()
                .await
                .expect("server should exit after SIGTERM");
        }
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.send_sigterm();
    }
}
