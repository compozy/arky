//! Criterion benchmark for subprocess spawn and shutdown latency.

use std::time::Duration;

use arky_provider::{
    ProcessConfig,
    ProcessManager,
};
use criterion::{
    Criterion,
    criterion_group,
    criterion_main,
};
use tokio::runtime::Runtime;

fn spawn_latency_benchmark(c: &mut Criterion) {
    let runtime = Runtime::new().expect("benchmark runtime should construct");
    let mut group = c.benchmark_group("spawn_latency");
    group.measurement_time(Duration::from_secs(8));
    group.sample_size(25);

    group.bench_function("spawn_and_shutdown_shell", |b| {
        b.to_async(&runtime).iter(|| async {
            let manager = ProcessManager::new(shell_process_config());
            let mut process = manager.spawn().expect("process should spawn");
            process
                .graceful_shutdown()
                .await
                .expect("process should shut down");
        });
    });

    group.finish();
}

#[cfg(unix)]
fn shell_process_config() -> ProcessConfig {
    ProcessConfig::new("sh")
        .with_args(["-c", "cat >/dev/null"])
        .with_shutdown_timeout(Duration::from_millis(100))
}

#[cfg(windows)]
fn shell_process_config() -> ProcessConfig {
    ProcessConfig::new("cmd")
        .with_args(["/C", "more > NUL"])
        .with_shutdown_timeout(Duration::from_millis(100))
}

criterion_group!(benches, spawn_latency_benchmark);
criterion_main!(benches);
