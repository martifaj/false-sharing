//! False sharing demo for "Low-Latency Rust, Ep. 1".
//!
//! Each thread increments ITS OWN counter. No data is logically shared,
//! yet the "packed" layout is much slower, because the counters live on
//! the same cache line and the cores keep stealing it from each other.
//!
//! Run with:  cargo run --release            (defaults to 2 threads)
//!            cargo run --release -- 4       (choose thread count)

use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const ITERS: u64 = 50_000_000;
const RUNS: usize = 7;

/// Layout 1: counters packed next to each other.
/// 8 bytes each, so up to 8 of them share one 64-byte cache line.
struct Packed {
    counters: Vec<AtomicU64>,
}

/// Layout 2: every counter gets its own cache line.
/// 128 rather than 64: modern Intel cores prefetch lines in adjacent
/// pairs, and Apple M-series cores use 128-byte lines. This is the same
/// value crossbeam's `CachePadded` uses on x86_64 and aarch64.
#[repr(align(128))]
struct Padded(AtomicU64);

struct Spread {
    counters: Vec<Padded>,
}

fn bench<F>(threads: usize, work: F) -> Duration
where
    F: Fn(usize) + Sync,
{
    let start = Instant::now();
    thread::scope(|s| {
        for t in 0..threads {
            let work = &work;
            s.spawn(move || work(t));
        }
    });
    start.elapsed()
}

fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v[v.len() / 2]
}

fn main() {
    let threads: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(2);

    let line = std::mem::size_of::<Padded>();
    println!("threads: {threads}, increments per thread: {ITERS}, runs: {RUNS} (median shown)");
    println!("size_of::<AtomicU64>() = 8 bytes, size_of::<Padded>() = {line} bytes\n");

    // 1. Packed: neighbours on the same cache line.
    let packed = Packed {
        counters: (0..threads).map(|_| AtomicU64::new(0)).collect(),
    };
    let t_packed = median(
        (0..RUNS)
            .map(|_| {
                bench(threads, |t| {
                    let c = &packed.counters[t];
                    for _ in 0..ITERS {
                        c.fetch_add(1, Ordering::Relaxed);
                    }
                })
            })
            .collect(),
    );

    // 2. Padded: one cache line per counter.
    let spread = Spread {
        counters: (0..threads).map(|_| Padded(AtomicU64::new(0))).collect(),
    };
    let t_padded = median(
        (0..RUNS)
            .map(|_| {
                bench(threads, |t| {
                    let c = &spread.counters[t].0;
                    for _ in 0..ITERS {
                        c.fetch_add(1, Ordering::Relaxed);
                    }
                })
            })
            .collect(),
    );

    // 3. Local: count in a register, publish once at the end.
    //    Often the best fix: don't touch shared memory in the hot loop.
    let local = Packed {
        counters: (0..threads).map(|_| AtomicU64::new(0)).collect(),
    };
    let t_local = median(
        (0..RUNS)
            .map(|_| {
                bench(threads, |t| {
                    let mut n = 0u64;
                    for _ in 0..ITERS {
                        n = std::hint::black_box(n + 1);
                    }
                    local.counters[t].fetch_add(n, Ordering::Relaxed);
                })
            })
            .collect(),
    );

    // Sanity check: every layout counted everything.
    let expect = ITERS * threads as u64 * RUNS as u64;
    let sum = |v: &mut dyn Iterator<Item = u64>| v.sum::<u64>();
    assert_eq!(sum(&mut packed.counters.iter().map(|c| c.load(Ordering::Relaxed))), expect);
    assert_eq!(sum(&mut spread.counters.iter().map(|c| c.0.load(Ordering::Relaxed))), expect);
    assert_eq!(sum(&mut local.counters.iter().map(|c| c.load(Ordering::Relaxed))), expect);

    let ns = |d: Duration| d.as_nanos() as f64 / ITERS as f64;
    println!("{:<34}{:>10}{:>14}", "layout", "time", "ns / incr");
    println!("{:<34}{:>8.0}ms{:>14.2}", "packed (same cache line)", t_packed.as_secs_f64() * 1e3, ns(t_packed));
    println!("{:<34}{:>8.0}ms{:>14.2}", "padded (own cache line)", t_padded.as_secs_f64() * 1e3, ns(t_padded));
    println!("{:<34}{:>8.0}ms{:>14.2}", "local (publish once at end)", t_local.as_secs_f64() * 1e3, ns(t_local));
    println!(
        "\npacked is {:.1}x slower than padded",
        t_packed.as_secs_f64() / t_padded.as_secs_f64()
    );
}
