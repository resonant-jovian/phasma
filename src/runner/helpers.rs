//! Shared helpers for runner modules (sweep, convergence, regression).

use std::sync::Arc;

use crate::sim::SimState;
use crate::sim::StateReceiver;

/// Compute the default concurrency limit: configured value, or `available_parallelism / 2`
/// (minimum 1), falling back to 2 if the OS query fails.
pub fn default_concurrency(configured: Option<usize>) -> usize {
    configured.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| (n.get() / 2).max(1))
            .unwrap_or(2)
    })
}

/// Drain the state receiver to completion, printing log messages with the given `label`,
/// and return the final `SimState`.
///
/// This is the common "drain loop" used by sweep, convergence, and regression runners:
/// receive every state update, forward log lines to stderr, and capture the last state.
pub async fn drain_to_final(state_rx: &mut StateReceiver, label: &str) -> Option<Arc<SimState>> {
    let mut final_state = None;
    while let Some(state) = state_rx.recv_async().await {
        for msg in &state.log_messages {
            eprintln!("  [{label}] {msg}");
        }
        let is_exit = state.exit_reason.is_some();
        final_state = Some(state);
        if is_exit {
            break;
        }
    }
    final_state
}
