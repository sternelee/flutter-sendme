pub mod api;
pub mod sendme_core;
mod frb_generated;

use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use std::collections::HashMap;

pub type ProgressSender = Arc<Mutex<Option<mpsc::UnboundedSender<ProgressInfo>>>>;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProgressInfo {
    pub operation: ProgressOperation,
    pub current: u64,
    pub total: u64,
    pub message: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ProgressOperation {
    Import,
    Export,
    Download,
    Connect,
}

// Stream-based progress callback for real-time updates
#[flutter_rust_bridge::frb]
pub struct ProgressStream {
    #[frb(ignore)]
    pub receiver: Option<mpsc::UnboundedReceiver<ProgressInfo>>,
}

impl ProgressStream {
    pub fn new() -> (Self, ProgressSender) {
        let (sender, receiver) = mpsc::unbounded_channel();
        let stream = ProgressStream {
            receiver: Some(receiver),
        };
        let sender = Arc::new(Mutex::new(Some(sender)));
        (stream, sender)
    }

    pub fn try_recv(&mut self) -> Option<ProgressInfo> {
        if let Some(ref mut receiver) = self.receiver {
            receiver.try_recv().ok()
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SendResult {
    pub ticket: String,
    pub hash: String,
    pub size: u64,
    pub file_count: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReceiveResult {
    pub file_count: u64,
    pub size: u64,
    pub duration_ms: u64,
}

// Simplified global state to keep senders alive
use std::any::Any;

pub struct SendmeState {
    pub senders: Arc<Mutex<HashMap<String, Box<dyn Any + Send + Sync>>>>,
}

impl SendmeState {
    pub fn new() -> Self {
        Self {
            senders: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn add_sender(&self, ticket: String, sender: Box<dyn Any + Send + Sync>) {
        let mut senders = self.senders.lock().unwrap();
        senders.insert(ticket, sender);
    }

    pub fn remove_sender(&self, ticket: &str) {
        let mut senders = self.senders.lock().unwrap();
        senders.remove(ticket);
    }
}

lazy_static::lazy_static! {
    pub static ref SENDME_STATE: SendmeState = SendmeState::new();
}
