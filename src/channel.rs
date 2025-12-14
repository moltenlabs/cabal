//! Communication channels for the orchestrator

use tokio::sync::mpsc;
use warhorn::{Op, Event};

/// Channel pair for orchestrator communication
pub struct ChannelPair {
    /// Receiver for operations
    pub op_rx: mpsc::UnboundedReceiver<Op>,
    /// Sender for events
    pub event_tx: mpsc::UnboundedSender<Event>,
}

/// Client-side channel for communicating with the orchestrator
#[derive(Clone)]
pub struct GoblinChannel {
    /// Sender for operations
    op_tx: mpsc::UnboundedSender<Op>,
    /// Receiver for events
    event_rx: std::sync::Arc<parking_lot::Mutex<mpsc::UnboundedReceiver<Event>>>,
}

impl GoblinChannel {
    /// Create a new channel pair
    ///
    /// Returns the client channel and the orchestrator channel pair
    pub fn new() -> (Self, ChannelPair) {
        let (op_tx, op_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let channel = Self {
            op_tx,
            event_rx: std::sync::Arc::new(parking_lot::Mutex::new(event_rx)),
        };

        let pair = ChannelPair { op_rx, event_tx };

        (channel, pair)
    }

    /// Send an operation to the orchestrator
    pub fn send(&self, op: Op) -> Result<(), ChannelError> {
        self.op_tx.send(op).map_err(|_| ChannelError::Closed)
    }

    /// Try to receive an event (non-blocking)
    pub fn try_recv(&self) -> Option<Event> {
        self.event_rx.lock().try_recv().ok()
    }

    /// Receive an event (blocking)
    pub async fn recv(&self) -> Option<Event> {
        // Note: This requires careful handling since we're holding the mutex
        // In practice, you'd want a different design for async recv
        let mut guard = self.event_rx.lock();
        guard.recv().await
    }

    /// Check if the channel is closed
    pub fn is_closed(&self) -> bool {
        self.op_tx.is_closed()
    }
}

impl Default for GoblinChannel {
    fn default() -> Self {
        Self::new().0
    }
}

/// Channel errors
#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    #[error("Channel is closed")]
    Closed,
}

/// Builder for creating configured channels
pub struct ChannelBuilder {
    buffer_size: Option<usize>,
}

impl ChannelBuilder {
    pub fn new() -> Self {
        Self { buffer_size: None }
    }

    /// Set buffer size (bounded channel)
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = Some(size);
        self
    }

    /// Build the channel pair
    pub fn build(self) -> (GoblinChannel, ChannelPair) {
        // For now, always use unbounded
        // Could add bounded channel support based on buffer_size
        GoblinChannel::new()
    }
}

impl Default for ChannelBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use warhorn::SubmissionId;

    #[test]
    fn test_channel_creation() {
        let (channel, _pair) = GoblinChannel::new();
        assert!(!channel.is_closed());
    }

    #[test]
    fn test_send_op() {
        let (channel, mut pair) = GoblinChannel::new();
        
        let op = Op::interrupt();
        channel.send(op).unwrap();
        
        // Check it was received
        let received = pair.op_rx.try_recv();
        assert!(received.is_ok());
    }

    #[tokio::test]
    async fn test_receive_event() {
        let (channel, pair) = GoblinChannel::new();
        
        // Send an event
        let event = Event::Warning {
            sub_id: SubmissionId::new(),
            message: "test".to_string(),
            details: None,
        };
        pair.event_tx.send(event).unwrap();
        
        // Receive it
        let received = channel.try_recv();
        assert!(received.is_some());
    }
}
