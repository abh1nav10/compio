#![allow(dead_code, unused_variables, unused_assignments, unused_imports)]

use std::{
    hint::black_box,
    pin::{Pin, pin},
    sync::{Arc, Barrier},
    task::{Context, Poll, Waker},
};

use compio_executor::{Executor, JoinError, JoinHandle};
use criterion::{Criterion, criterion_group, criterion_main};
use flume::{Receiver, Sender, bounded};

std::thread_local! {
    static EXE: Executor = Executor::new();
}

fn block_on<F: Future + 'static>(f: F) -> F::Output {
    EXE.with(|exe| {
        let cx = &mut Context::from_waker(Waker::noop());
        let mut f = pin!(f);
        loop {
            if let Poll::Ready(res) = f.as_mut().poll(cx) {
                return res;
            }
            exe.tick();
        }
    })
}

struct CounterFuture {
    desired: usize,
    current: usize,
    shared: Sender<Waker>,
}

impl CounterFuture {
    fn new(desired: usize, sender: Sender<Waker>) -> Self {
        Self {
            desired,
            current: 0,
            shared: sender,
        }
    }
}

impl Future for CounterFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.current += 1;
        if self.current < self.desired {
            self.shared.send(cx.waker().clone()).expect(
                "Must always succeed because we only push when we have been woken which implies \
                 that the previous send has been received",
            );

            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

macro_rules! gen_func {
    ($name:ident, $num:expr) => {
        fn $name(c: &mut Criterion) {
            let (tx, rx) = bounded::<Waker>(1);
            std::thread::spawn(move || {
                loop {
                    if let Ok(t) = rx.recv() {
                        t.wake();
                    }
                }
            });

            c.bench_function("FastSync Remote", |b| {
                b.iter(|| {
                    let cloned = tx.clone();
                    block_on(black_box(async move {
                        let counterfuture = CounterFuture::new($num, cloned);
                        counterfuture.await;
                    }))
                })
            });
        }
    };
}

gen_func!(bench_remote10, 10);
gen_func!(bench_remote100, 100);
gen_func!(bench_remote1000, 1000);

criterion_group!(benches, bench_remote10, bench_remote100, bench_remote1000);
criterion_main!(benches);
