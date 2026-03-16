//! Small actor-style command queue used by the agent runtime.

#[cfg(test)]
use std::future::Future;

use tokio::sync::mpsc;

use crate::CoreError;

/// Actor-style queue that serializes command handling through one task.
pub struct CommandQueue<C> {
    sender: mpsc::UnboundedSender<C>,
}

impl<C> Clone for CommandQueue<C> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<C> CommandQueue<C>
where
    C: Send + 'static,
{
    /// Wraps an existing sender as a queue handle.
    pub const fn from_sender(sender: mpsc::UnboundedSender<C>) -> Self {
        Self { sender }
    }

    /// Starts the queue and spawns the command-processing task.
    #[cfg(test)]
    pub fn start<F, Fut>(mut handler: F) -> Self
    where
        F: FnMut(C) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            while let Some(command) = receiver.recv().await {
                handler(command).await;
            }
        });

        Self { sender }
    }

    /// Enqueues a command for serialized handling.
    pub fn send(&self, command: C) -> Result<(), CoreError> {
        self.sender.send(command).map_err(|_| {
            CoreError::invalid_state("agent command queue is no longer available", None)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use pretty_assertions::assert_eq;
    use tokio::sync::{
        Mutex,
        oneshot,
    };

    use super::CommandQueue;

    #[tokio::test]
    async fn command_queue_should_serialize_concurrent_submits() {
        let observed = Arc::new(Mutex::new(Vec::new()));
        let queue = CommandQueue::start({
            let observed = Arc::clone(&observed);
            move |(value, done): (u8, oneshot::Sender<()>)| {
                let observed = Arc::clone(&observed);
                async move {
                    observed.lock().await.push(value);
                    let _ = done.send(());
                }
            }
        });

        let mut waiters = Vec::new();
        for value in 0_u8..4 {
            let (done_tx, done_rx) = oneshot::channel();
            queue
                .send((value, done_tx))
                .expect("queue should accept command");
            waiters.push(done_rx);
        }

        for waiter in waiters {
            waiter.await.expect("handler should acknowledge command");
        }

        let actual = observed.lock().await.clone();
        let expected = vec![0, 1, 2, 3];
        assert_eq!(actual, expected);
    }
}
