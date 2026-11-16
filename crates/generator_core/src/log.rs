use std::sync::Arc;

/// A cheap, cloneable callback used to surface diagnostics while the generator is running.
///
/// Release builds use the `windows` subsystem, so there is no console and anything written to
/// stderr is silently lost. The GUI installs a sink that appends every message to its messages
/// area; headless callers (tests, future CLI) leave it unset and fall back to stderr.
#[derive(Clone)]
pub struct LogSink(Arc<dyn Fn(String) + Send + Sync>);

impl LogSink {
    pub fn new(sink: impl Fn(String) + Send + Sync + 'static) -> Self {
        Self(Arc::new(sink))
    }

    pub fn log(&self, message: String) {
        (self.0)(message);
    }
}

impl std::fmt::Debug for LogSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LogSink")
    }
}
