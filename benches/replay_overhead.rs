//! Criterion benchmark for session snapshot and replay retrieval overhead.

use std::{
    hint::black_box,
    time::Duration,
};

use arky_protocol::{
    AgentEvent,
    EventMetadata,
    Message,
    PersistedEvent,
    SessionId,
    TurnCheckpoint,
    TurnId,
};
use arky_session::{
    InMemorySessionStore,
    NewSession,
    SessionStore,
};
use criterion::{
    BenchmarkId,
    Criterion,
    Throughput,
    criterion_group,
    criterion_main,
};
use tokio::runtime::Runtime;

fn replay_overhead_benchmark(c: &mut Criterion) {
    let runtime = Runtime::new().expect("benchmark runtime should construct");
    let mut group = c.benchmark_group("replay_overhead");
    group.measurement_time(Duration::from_secs(8));

    for event_count in [128_usize, 1_024, 4_096] {
        let (store, session_id) = runtime.block_on(seed_store(event_count));
        group.throughput(Throughput::Elements(event_count as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(event_count),
            &event_count,
            |b, &_event_count| {
                b.to_async(&runtime).iter(|| async {
                    let snapshot =
                        store.load(&session_id).await.expect("snapshot should load");
                    let events = store
                        .replay_events(&session_id, None, None)
                        .await
                        .expect("replay should load");
                    black_box((snapshot, events));
                });
            },
        );
    }

    group.finish();
}

async fn seed_store(event_count: usize) -> (InMemorySessionStore, SessionId) {
    let store = InMemorySessionStore::default();
    let session_id = store
        .create(NewSession::default())
        .await
        .expect("session should create");
    let turn_id = TurnId::new();

    let events = (0..event_count)
        .map(|index| {
            let event = AgentEvent::MessageUpdate {
                meta: EventMetadata::new(index as u64, index as u64 + 1)
                    .with_session_id(session_id.clone())
                    .with_turn_id(turn_id.clone()),
                message: Message::assistant(format!("chunk-{index}")),
                delta: arky_protocol::StreamDelta::text("delta"),
            };
            PersistedEvent::new(event)
        })
        .collect::<Vec<_>>();

    store
        .append_events(&session_id, &events)
        .await
        .expect("events should persist");
    store
        .save_turn_checkpoint(
            &session_id,
            TurnCheckpoint::new(turn_id, event_count as u64)
                .with_message(Message::assistant("done")),
        )
        .await
        .expect("checkpoint should persist");

    (store, session_id)
}

criterion_group!(benches, replay_overhead_benchmark);
criterion_main!(benches);
