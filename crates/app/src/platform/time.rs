//! Timers and background work that run on every target: tokio's timers and
//! real threads on the Mac and Android, the browser's (`setTimeout`, one
//! thread) on the web, where tokio has no timer and `std::thread` panics.
// adapted from batna-app main: crates/app/src/platform/time.rs

use std::future::Future;
use std::time::Duration;

/// Waits `d` without blocking the UI.
pub async fn sleep(d: Duration) {
    #[cfg(not(target_arch = "wasm32"))]
    tokio::time::sleep(d).await;
    #[cfg(target_arch = "wasm32")]
    gloo_timers::future::sleep(d).await;
}

/// `fut`'s output, or `Err(Elapsed)` after `limit`.
#[allow(dead_code)] // the web's twin of tokio::time::timeout, for callers that need one
pub async fn timeout<F: Future>(limit: Duration, fut: F) -> Result<F::Output, Elapsed> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        tokio::time::timeout(limit, fut).await.map_err(|_| Elapsed)
    }
    #[cfg(target_arch = "wasm32")]
    {
        use futures_util::future::{Either, select};
        let fut = std::pin::pin!(fut);
        let timer = std::pin::pin!(sleep(limit));
        match select(fut, timer).await {
            Either::Left((out, _)) => Ok(out),
            Either::Right(_) => Err(Elapsed),
        }
    }
}

/// The time limit ran out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Elapsed;

/// Runs `f` off the UI thread where there are threads (a named one); the web
/// has one thread, so `f` runs there on the next turn of the event loop,
/// after the current screen has painted. `Err` only when no thread started.
pub fn spawn(name: &str, f: impl FnOnce() + Send + 'static) -> Result<(), String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::Builder::new().name(name.into()).spawn(f).map(drop).map_err(|e| e.to_string())
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = name;
        wasm_bindgen_futures::spawn_local(async move {
            sleep(Duration::ZERO).await;
            f();
        });
        Ok(())
    }
}

/// Blocking work off the UI thread where there are threads; in place on the web.
pub async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    #[cfg(not(target_arch = "wasm32"))]
    {
        tokio::task::spawn_blocking(f).await.expect("blocking task panicked")
    }
    #[cfg(target_arch = "wasm32")]
    {
        f()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_gives_the_answer_in_time_and_elapsed_after() {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let quick = rt.block_on(timeout(Duration::from_secs(5), async { 7 }));
        assert_eq!(quick, Ok(7));
        let slow = rt.block_on(timeout(Duration::from_millis(10), sleep(Duration::from_secs(5))));
        assert_eq!(slow, Err(Elapsed));
    }

    #[test]
    fn spawn_runs_off_this_thread_and_blocking_returns_its_value() {
        let (tx, rx) = std::sync::mpsc::channel();
        let here = std::thread::current().id();
        spawn("kabar-test", move || tx.send(std::thread::current().id()).unwrap()).unwrap();
        let there = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_ne!(there, here);
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        assert_eq!(rt.block_on(blocking(|| 6 * 7)), 42);
    }
}
