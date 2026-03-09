#[derive(Debug, Clone, Copy)]
pub enum NotificationKind {
    SimComplete,
    SimError,
    CheckpointSaved,
    ExportComplete,
}

pub fn notify(kind: NotificationKind, msg: &str) {
    // Ring terminal bell
    print!("\x07");

    let _title = match kind {
        NotificationKind::SimComplete => "Phasma: Simulation complete",
        NotificationKind::SimError => "Phasma: Simulation error",
        NotificationKind::CheckpointSaved => "Phasma: Checkpoint saved",
        NotificationKind::ExportComplete => "Phasma: Export complete",
    };

    #[cfg(feature = "notifications")]
    {
        let _ = notify_rust::Notification::new()
            .summary(_title)
            .body(msg)
            .show();
    }

    let _ = msg; // suppress unused warning when feature is disabled
}
