#![allow(dead_code, unused_variables, unused_assignments, unused_imports)]

use std::{
    cell::{Cell, RefCell},
    future::Future,
    hint::black_box,
    io::empty,
    pin::{Pin, pin},
    sync::{Arc, Barrier},
    task::{Context, Poll, Waker},
};

use compio_executor::{Executor, JoinError, JoinHandle};
use criterion::{Criterion, criterion_group, criterion_main};

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
    count: usize,
    desired: usize,
}

impl CounterFuture {
    fn new(desired: usize) -> Self {
        Self { count: 0, desired }
    }
}

impl Future for CounterFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.count += 1;
        if self.count < self.desired {
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

fn measure(tasks: usize) {
    block_on(black_box(async move {
        (0..tasks).for_each(|_| {
            EXE.with(|exe| {
                exe.spawn(async move {
                    let counterfuture = CounterFuture::new(10);
                    counterfuture.await;
                })
            })
            .detach();
        });
        let counterfuture = CounterFuture::new(1000);
        counterfuture.await;
    }));
}

fn bench_empty(c: &mut Criterion) {
    c.bench_function("FastSync", |b| b.iter(|| measure(black_box(0))));
    c.bench_function("FastSync", |b| b.iter(|| measure(black_box(10))));
    c.bench_function("FastSync", |b| b.iter(|| measure(black_box(50))));
}

criterion_group!(benches, bench_empty);
criterion_main!(benches);
