//! A thread of its own, because the writes are file I/O.

use std::sync::Arc;

use tokio::sync::mpsc::UnboundedReceiver;
use velo_core::UiEvent;

use crate::state::StateHub;

/// The drain, kept joinable rather than detached.
pub(crate) struct Handle(std::thread::JoinHandle<()>);

impl Handle {
    /// Wait for the queue to drain.
    ///
    /// On a blocking thread, because joining one is: a runtime worker parked
    /// in a join is a worker not driving anything else.
    pub(crate) async fn join(self) {
        if let Err(e) = velo_session::unblock(move || self.0.join()).await {
            log::error!("the publish thread did not finish: {e}");
        }
    }
}

/// Start draining `queue` into `hub`.
pub(crate) fn start(hub: Arc<StateHub>, mut queue: UnboundedReceiver<UiEvent>) -> Handle {
    let thread = std::thread::Builder::new()
        .name("velo-publish".to_string())
        .spawn(move || {
            while let Some(event) = queue.blocking_recv() {
                super::publish_one(&hub, event);
            }
        })
        // A daemon that cannot spawn a thread is a daemon that will not get
        // far; failing here beats doing the writes on a worker.
        .expect("spawning the publish thread");
    Handle(thread)
}
