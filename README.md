# false-sharing

Benchmark from **Low-Latency Rust, Ep. 1: False Sharing**.

📺 Video: https://youtu.be/ecu1ocgCcFQ

Each thread increments **its own** atomic counter. No data is logically shared and there are no locks, yet one memory layout is several times slower than another. The counters sit on the same cache line, so the cores keep stealing that line from each other.

The benchmark compares three layouts:

| layout | what it does |
|---|---|
| **packed** | counters next to each other (`AtomicU64`, 8 bytes each), so they share one 64-byte cache line |
| **padded** | each counter in a `#[repr(align(128))]` wrapper, so every counter gets its own line |
| **local** | count in a local variable and publish once at the end, so shared memory isn't touched in the hot loop |

## Run it

Requires Rust 1.85 or newer (edition 2024). No dependencies.

```sh
cargo run --release          # 2 threads
cargo run --release -- 4     # 4 threads (or any other count)
```

Always use `--release`. A debug build measures something else entirely.

Each layout does 50 million increments per thread, 7 runs, and the median is reported. A full run takes about 20 seconds on the machine below.

## Results from the video

Raspberry Pi 5, 4x Cortex-A76 @ 3.0 GHz (overclocked; stock is 2.4 GHz), `performance` governor, 64-byte cache lines.

```text
threads: 2, increments per thread: 50000000, runs: 7 (median shown)
size_of::<AtomicU64>() = 8 bytes, size_of::<Padded>() = 128 bytes

layout                                  time     ns / incr
packed (same cache line)              2276ms         45.52
padded (own cache line)                240ms          4.80
local (publish once at end)             92ms          1.84

packed is 9.5x slower than padded
```

| threads | packed | padded | local | packed vs padded |
|---|---|---|---|---|
| 2 | 45.5 ns | 4.8 ns | 1.8 ns | 9.5x |
| 4 | 90.9 ns | 4.9 ns | 1.9 ns | 18.6x |

Run-to-run variation on this machine was about 2% (between 9.5x and 9.7x for 2 threads).

**Your numbers will differ.** Core count, cache layout, and whether the cores share a die or sit across chiplets or sockets all change the cost of moving a line between cores. Open an issue or leave a comment on the video with your CPU and ratio.

## Notes

- **Why 128 and not 64?** Many Intel cores prefetch cache lines in adjacent pairs, so two neighbouring 64-byte lines can still interfere. Apple M-series chips use 128-byte lines. 128 is also what [`crossbeam_utils::CachePadded`](https://docs.rs/crossbeam-utils/latest/crossbeam_utils/struct.CachePadded.html) uses on x86-64 and aarch64. In real code, use that type rather than rolling your own.
- **The local layout** uses `std::hint::black_box`, so the compiler can't fold the loop into a single addition. Without it, that number would be meaningless.
- **Don't pad everything.** Padding wastes memory and cache space. Measure first. On Linux, `perf c2c` can point straight at false sharing.
- At the end, the program checks that every layout counted every increment.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
