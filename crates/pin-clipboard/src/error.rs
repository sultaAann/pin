#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("clipboard access is not supported on this platform")]
    Unsupported,
    #[error("no format can be written to the clipboard")]
    NothingToWrite,
    #[error("clipboard rejected the write")]
    WriteRejected,
    #[error("failed to start the clipboard watcher")]
    Spawn(#[source] std::io::Error),
    #[error("clipboard watcher stopped unexpectedly")]
    WatcherStopped,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
