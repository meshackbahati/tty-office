# tty-office performance and memory budgets

Budgets are claims about the optimized build, enforced by the
ignored tests in `tests/perf_budget.rs`. Run them explicitly with:

```sh
cargo test --release --test perf_budget -- --ignored --nocapture --test-threads=1
```

They stay ignored by default because the hundred-megabyte load
would otherwise run on every development cycle. The single thread
is load-bearing rather than stylistic: resident measurements read
process-wide RSS, so parallel tests would charge each other for
their allocations.

## Time budgets

| Workload | Budget | Measured |
|----------|--------|----------|
| Mean frame, 10k-line document | under 16.6 ms | _see run_ |
| Mean keystroke, 10k-line buffer | under 50 us | _see run_ |
| Long-line insert sanity | under 2 ms | _recorded only_ |

## Memory budgets

| Workload | Budget | Measured |
|----------|--------|----------|
| 100 MB text load, RSS delta | under 160 MB | _see run_ |
| 100 MB text load, absolute RSS | under 320 MB | _see run_ |
| 2k-row formula workbook open, RSS delta | under 150 MB | _see run_ |
| 5 MB word document open, RSS delta | under 300 MB | _see run_ |

RSS deltas come from `/proc/self/status` around the open call, so
they include the allocator and harness overhead of the test
process; the budgets allow headroom above the structures
themselves. Resident numbers vary between runs, and the printed
`before/after/delta` lines in the test output are the record of
any single run rather than a standing claim.
