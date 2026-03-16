//! Criterion benchmark for provider event stream throughput.

use std::{
    hint::black_box,
    time::Duration,
};

use arky_protocol::{
    AgentEvent,
    EventMetadata,
    Message,
    SessionId,
    SessionRef,
    StreamDelta,
    TurnContext,
    TurnId,
};
use arky_provider::{
    ProviderEventStream,
    generate_response_from_stream,
};
use criterion::{
    BenchmarkId,
    Criterion,
    Throughput,
    criterion_group,
    criterion_main,
};
use futures::stream;
use tokio::runtime::Runtime;

fn event_throughput_benchmark(c: &mut Criterion) {
    let runtime = Runtime::new().expect("benchmark runtime should construct");
    let mut group = c.benchmark_group("event_throughput");
    group.measurement_time(Duration::from_secs(8));

    for event_count in [64_usize, 512, 4_096] {
        group.throughput(Throughput::Elements(event_count as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(event_count),
            &event_count,
            |b, &event_count| {
                b.to_async(&runtime).iter(|| async move {
                    let session_id = SessionId::new();
                    let turn_id = TurnId::new();
                    let session = SessionRef::new(Some(session_id.clone()));
                    let turn = TurnContext::new(turn_id.clone(), 1);
                    let stream = build_stream(&session_id, &turn_id, event_count);

                    let response = generate_response_from_stream(session, turn, stream)
                        .await
                        .expect("stream should produce a terminal response");
                    black_box(response);
                });
            },
        );
    }

    group.finish();
}

fn build_stream(
    session_id: &SessionId,
    turn_id: &TurnId,
    event_count: usize,
) -> ProviderEventStream {
    let total_events = event_count.max(2);
    let mut events = Vec::with_capacity(total_events);
    let final_index = total_events - 1;
    let partial_message = Message::assistant("");
    let final_message = Message::assistant("stream-complete");

    for index in 0..total_events {
        let meta = EventMetadata::new(index as u64, index as u64 + 1)
            .with_session_id(session_id.clone())
            .with_turn_id(turn_id.clone());

        let event = if index == 0 {
            AgentEvent::MessageStart {
                meta,
                message: partial_message.clone(),
            }
        } else if index == final_index {
            AgentEvent::TurnEnd {
                meta,
                message: final_message.clone(),
                tool_results: Vec::new(),
            }
        } else {
            AgentEvent::MessageUpdate {
                meta,
                message: partial_message.clone(),
                delta: StreamDelta::text("delta"),
            }
        };
        events.push(Ok(event));
    }

    Box::pin(stream::iter(events))
}

criterion_group!(benches, event_throughput_benchmark);
criterion_main!(benches);
