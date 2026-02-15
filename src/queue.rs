// Notification queue - cross-process coordination for sequential playback

use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use nix::fcntl::Flock;
use nix::fcntl::FlockArg;

use crate::error::{Result, VoiceError};

/// Notification queue for cross-process coordination
pub struct NotificationQueue {
    lock_file_path: PathBuf,
    timeout: Duration,
}

impl NotificationQueue {
    /// Create new queue instance
    ///
    /// # Arguments
    /// * `timeout` - Maximum wait time to acquire lock (default 30s)
    pub fn new(timeout: Option<Duration>) -> Result<Self> {
        let lock_file_path = Self::default_lock_path()?;
        let timeout = timeout.unwrap_or(Duration::from_secs(30));

        Ok(Self {
            lock_file_path,
            timeout,
        })
    }

    /// Get default lock file path: ~/.sumvox/queue.lock
    fn default_lock_path() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| VoiceError::Config("Cannot find home directory".into()))?;
        Ok(home.join(".sumvox").join("queue.lock"))
    }

    /// Ensure lock directory exists
    fn ensure_lock_dir(&self) -> Result<()> {
        if let Some(parent) = self.lock_file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                VoiceError::Queue(format!("Failed to create lock directory: {}", e))
            })?;
        }
        Ok(())
    }
}

/// RAII wrapper for queue lock
pub struct QueueLock {
    _flock: Flock<File>,
}

impl QueueLock {
    /// Acquire exclusive lock on notification queue
    ///
    /// Blocks until lock available or timeout exceeded.
    ///
    /// # Arguments
    /// * `queue` - Queue instance
    ///
    /// # Returns
    /// Ok(QueueLock) if acquired within timeout
    /// Err if timeout exceeded or lock file inaccessible
    pub async fn acquire(queue: &NotificationQueue) -> Result<Self> {
        let start_time = Instant::now();

        // Ensure lock directory exists
        queue.ensure_lock_dir()?;

        // Try to acquire lock with timeout
        loop {
            // Open or create lock file
            let file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(false)
                .open(&queue.lock_file_path)
                .map_err(|e| VoiceError::Queue(format!("Failed to open lock file: {}", e)))?;

            // Try non-blocking lock
            match Flock::lock(file, FlockArg::LockExclusiveNonblock) {
                Ok(flock) => {
                    // Lock acquired!
                    let elapsed = start_time.elapsed();
                    tracing::info!("Queue lock acquired after {:?}", elapsed);

                    return Ok(QueueLock {
                        _flock: flock,
                    });
                }
                Err((_, nix::errno::Errno::EWOULDBLOCK)) => {
                    // Lock is held by another process
                    let elapsed = start_time.elapsed();

                    if elapsed >= queue.timeout {
                        tracing::warn!(
                            "Queue lock timeout ({:?}) exceeded, skipping notification",
                            queue.timeout
                        );
                        return Err(VoiceError::Queue(format!(
                            "Lock acquisition timeout after {:?}",
                            elapsed
                        )));
                    }

                    // Sleep briefly before retry
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err((_, e)) => {
                    return Err(VoiceError::Queue(format!("Failed to acquire lock: {}", e)));
                }
            }
        }
    }
}

impl Drop for QueueLock {
    fn drop(&mut self) {
        // Release lock (Flock automatically releases on drop)
        tracing::debug!("Queue lock released");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_queue_creation() {
        let queue = NotificationQueue::new(Some(Duration::from_secs(10))).unwrap();
        assert_eq!(queue.timeout, Duration::from_secs(10));
        assert!(queue.lock_file_path.to_string_lossy().contains(".sumvox"));
    }

    #[test]
    fn test_queue_default_timeout() {
        let queue = NotificationQueue::new(None).unwrap();
        assert_eq!(queue.timeout, Duration::from_secs(30));
    }

    #[tokio::test]
    async fn test_lock_acquire_when_available() {
        let temp_dir = tempdir().unwrap();
        let lock_path = temp_dir.path().join("test.lock");

        // Create queue with custom lock path
        let mut queue = NotificationQueue::new(Some(Duration::from_secs(5))).unwrap();
        queue.lock_file_path = lock_path;

        // Should acquire lock successfully
        let lock = QueueLock::acquire(&queue).await;
        assert!(lock.is_ok());
    }

    #[tokio::test]
    async fn test_lock_auto_release_on_drop() {
        let temp_dir = tempdir().unwrap();
        let lock_path = temp_dir.path().join("test.lock");

        let mut queue = NotificationQueue::new(Some(Duration::from_secs(5))).unwrap();
        queue.lock_file_path = lock_path.clone();

        {
            let _lock = QueueLock::acquire(&queue).await.unwrap();
            // Lock held here
        }
        // Lock released when _lock dropped

        // Should be able to acquire again
        let lock2 = QueueLock::acquire(&queue).await;
        assert!(lock2.is_ok());
    }

    #[test]
    fn test_ensure_lock_dir_creates_directory() {
        let temp_dir = tempdir().unwrap();
        let lock_path = temp_dir.path().join("subdir").join("test.lock");

        let mut queue = NotificationQueue::new(Some(Duration::from_secs(5))).unwrap();
        queue.lock_file_path = lock_path.clone();

        queue.ensure_lock_dir().unwrap();

        assert!(lock_path.parent().unwrap().exists());
    }
}
