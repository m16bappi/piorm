use once_cell::sync::OnceCell;
use tokio::runtime::Runtime;

static TOKIO_RUNTIME: OnceCell<Runtime> = OnceCell::new();

pub fn runtime() -> &'static Runtime {
    TOKIO_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime")
    })
}

/// Block on a future using the global Tokio runtime.
/// Used only in CLI context where we cannot be async.
pub fn block_on<F: std::future::Future>(f: F) -> F::Output {
    runtime().block_on(f)
}
