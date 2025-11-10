use crate::{
    sendme_core::{
        format_bytes as core_format_bytes, receive_file as core_receive_file,
        send_file as core_send_file,
    },
    ProgressInfo, ProgressSender, ReceiveResult, SendResult,
};
use flutter_rust_bridge::frb;

// Initialize logging
#[frb(sync)]
pub fn init_logging() {
    crate::sendme_core::init_logging();
}

// Send a file or directory
#[frb]
pub async fn send_file(path: String) -> anyhow::Result<SendResult> {
    core_send_file(path).await
}

// Receive a file or directory
#[frb]
pub async fn receive_file(ticket: String) -> anyhow::Result<ReceiveResult> {
    core_receive_file(ticket).await
}

// Format bytes for display
#[frb(sync)]
pub fn format_bytes(size: u64) -> String {
    core_format_bytes(size)
}

