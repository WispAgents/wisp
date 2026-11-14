//! Running a detect's probes at once (ADR-0011): a CLI's `--version`, `--help` and `models` each wait up to
//! the probe's time limit (15 s) when the CLI hangs, so one after another a detect waited for their sum (round 212:
//! opencode's three probes, 45 s). Pure future combinators, no runtime: the host runs each probe.

use std::future::Future;
use std::pin::pin;
use std::task::Poll;

/// Both at once; their two answers.
pub async fn both<A, B>(left: impl Future<Output = A>, right: impl Future<Output = B>) -> (A, B) {
    let (mut left, mut right) = (pin!(left), pin!(right));
    let (mut got_left, mut got_right) = (None, None);
    std::future::poll_fn(move |cx| {
        if got_left.is_none()
            && let Poll::Ready(v) = left.as_mut().poll(cx)
        {
            got_left = Some(v);
        }
        if got_right.is_none()
            && let Poll::Ready(v) = right.as_mut().poll(cx)
        {
            got_right = Some(v);
        }
        match (got_left.take(), got_right.take()) {
            (Some(l), Some(r)) => Poll::Ready((l, r)),
            (l, r) => {
                (got_left, got_right) = (l, r);
                Poll::Pending
            }
        }
    })
    .await
}

/// `first` (the probe that says whether the CLI is there: `--version`) and `rest` (what is only read when it is)
/// at once. `first` answering None ends it at once: `rest` is dropped (its probes ended) and never waited for.
pub async fn gated<A, B>(first: impl Future<Output = Option<A>>, rest: impl Future<Output = B>) -> Option<(A, B)> {
    let (mut first, mut rest) = (pin!(first), pin!(rest));
    let (mut got_first, mut got_rest) = (None, None);
    std::future::poll_fn(move |cx| {
        if got_first.is_none()
            && let Poll::Ready(v) = first.as_mut().poll(cx)
        {
            match v {
                Some(v) => got_first = Some(v),
                None => return Poll::Ready(None),
            }
        }
        if got_rest.is_none()
            && let Poll::Ready(v) = rest.as_mut().poll(cx)
        {
            got_rest = Some(v);
        }
        match (got_first.take(), got_rest.take()) {
            (Some(f), Some(r)) => Poll::Ready(Some((f, r))),
            (f, r) => {
                (got_first, got_rest) = (f, r);
                Poll::Pending
            }
        }
    })
    .await
}

/// `first`, else `fallback`, both started at once: `first` answering Some ends it (`fallback` dropped, its probe
/// ended); None waits for `fallback` (opencode's plain `models`, read only when `models --verbose` lists nothing).
pub async fn or_else<A>(first: impl Future<Output = Option<A>>, fallback: impl Future<Output = Option<A>>) -> Option<A> {
    let (mut first, mut fallback) = (pin!(first), pin!(fallback));
    let (mut first_done, mut got_fallback) = (false, None);
    std::future::poll_fn(move |cx| {
        if !first_done && let Poll::Ready(v) = first.as_mut().poll(cx) {
            if v.is_some() {
                return Poll::Ready(v);
            }
            first_done = true;
        }
        if got_fallback.is_none()
            && let Poll::Ready(v) = fallback.as_mut().poll(cx)
        {
            got_fallback = Some(v);
        }
        if first_done && let Some(v) = got_fallback.take() {
            return Poll::Ready(v);
        }
        Poll::Pending
    })
    .await
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::task::Context;

    /// Pending `n` times, then `v`.
    async fn after<T>(n: usize, v: T) -> T {
        let mut left = n;
        std::future::poll_fn(move |cx| {
            if left == 0 {
                return Poll::Ready(());
            }
            left -= 1;
            cx.waker().wake_by_ref();
            Poll::Pending
        })
        .await;
        v
    }

    fn run<F: Future>(f: F) -> (F::Output, usize) {
        let mut cx = Context::from_waker(std::task::Waker::noop());
        let mut f = pin!(f);
        let mut polls = 0;
        loop {
            polls += 1;
            if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
                return (v, polls);
            }
        }
    }

    /// One after another they'd take 3 + 5 polls; at once, the slower one's 5 (+1 to finish).
    #[test]
    fn both_runs_the_two_at_once() {
        let (v, polls) = run(both(after(3, 1), after(5, "b")));
        assert_eq!(v, (1, "b"));
        assert_eq!(polls, 6);
    }

    #[test]
    fn gated_waits_for_rest_only_when_first_answers() {
        let (v, polls) = run(gated(after(1, Some(7)), after(5, "help")));
        assert_eq!(v, Some((7, "help")));
        assert_eq!(polls, 6, "rest ran while first did");
    }

    #[test]
    fn or_else_takes_first_without_waiting_for_the_fallback() {
        let (v, polls) = run(or_else(after(1, Some("verbose")), after(1_000, Some("plain"))));
        assert_eq!((v, polls), (Some("verbose"), 2));
    }

    #[test]
    fn or_else_takes_the_fallback_when_first_has_nothing() {
        let (v, polls) = run(or_else(after(3, None), after(2, Some("plain"))));
        assert_eq!((v, polls), (Some("plain"), 4), "the fallback ran alongside first");
    }

    /// No CLI: rest is dropped (ending its probes), not waited for.
    #[test]
    fn gated_drops_rest_when_first_finds_nothing() {
        struct Flag(Arc<AtomicBool>);
        impl Drop for Flag {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let dropped = Arc::new(AtomicBool::new(false));
        let flag = Flag(dropped.clone());
        let rest = async move {
            let _f = flag;
            after(1_000, ()).await;
        };
        let (v, polls) = run(gated(after(2, None::<u8>), rest));
        assert_eq!(v, None);
        assert_eq!(polls, 3);
        assert!(dropped.load(Ordering::SeqCst), "rest's probes were left running");
    }
}
