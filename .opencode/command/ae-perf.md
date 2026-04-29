# Performance Tuning

We need to improve taffy performance.

I have set sysctl kernel.perf_event_paranoid=2.

There is a suite of benchmarks in ./benches that you may run, but as these take some time you should intially validate your code changes with targetting before/after microbenchmarks.

Explore the codebase, plan an improvement, write the microbench before and after tests, validate, ???, profit!