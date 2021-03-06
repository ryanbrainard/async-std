use std::cell::RefCell;
use std::future::Future;

static GLOBAL_EXECUTOR: once_cell::sync::Lazy<async_executor::Executor> = once_cell::sync::Lazy::new(async_executor::Executor::new);

thread_local! {
    static EXECUTOR: RefCell<async_executor::LocalExecutor> = RefCell::new(async_executor::LocalExecutor::new());
}

pub(crate) fn spawn<F, T>(future: F) -> async_executor::Task<T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    GLOBAL_EXECUTOR.spawn(future)
}

#[cfg(feature = "unstable")]
pub(crate) fn local<F, T>(future: F) -> async_executor::Task<T>
where
    F: Future<Output = T> + 'static,
    T: 'static,
{
    EXECUTOR.with(|executor| executor.borrow().spawn(future))
}

pub(crate) fn run<F, T>(future: F) -> T
where
    F: Future<Output = T>,
{
    EXECUTOR.with(|executor| enter(|| GLOBAL_EXECUTOR.enter(|| executor.borrow().run(future))))
}

pub(crate) fn run_global<F, T>(future: F) -> T
where
    F: Future<Output = T>,
{
    enter(|| GLOBAL_EXECUTOR.run(future))
}

/// Enters the tokio context if the `tokio` feature is enabled.
fn enter<T>(f: impl FnOnce() -> T) -> T {
    #[cfg(not(feature = "tokio02"))]
    return f();

    #[cfg(feature = "tokio02")]
    {
        use std::cell::Cell;
        use tokio::runtime::Runtime;

        thread_local! {
            /// The level of nested `enter` calls we are in, to ensure that the outermost always
            /// has a runtime spawned.
            static NESTING: Cell<usize> = Cell::new(0);
        }

        /// The global tokio runtime.
        static RT: once_cell::sync::Lazy<Runtime> = once_cell::sync::Lazy::new(|| Runtime::new().expect("cannot initialize tokio"));

        NESTING.with(|nesting| {
            let res = if nesting.get() == 0 {
                nesting.replace(1);
                RT.enter(f)
            } else {
                nesting.replace(nesting.get() + 1);
                f()
            };
            nesting.replace(nesting.get() - 1);
            res
        })
    }
}
