//! # 07 Event Streaming
//!
//! Demonstrates live event consumption with `subscribe()` while a prompt is
//! executing.

mod common;

use arky::{
    AgentEvent,
    EventMetadata,
    ProviderCapabilities,
    ProviderDescriptor,
    ProviderError,
    ProviderEventStream,
    ProviderFamily,
    ProviderId,
    ProviderRequest,
    StreamDelta,
    prelude::*,
};
use async_trait::async_trait;
use common::{
    ExampleError,
    describe_event,
    text_from_message,
};
use futures::stream;

#[derive(Clone)]
struct StreamingProvider {
    descriptor: ProviderDescriptor,
}

impl StreamingProvider {
    fn new() -> Self {
        Self {
            descriptor: ProviderDescriptor::new(
                ProviderId::new("stream-demo"),
                ProviderFamily::Custom("stream-demo".to_owned()),
                ProviderCapabilities::new()
                    .with_streaming(true)
                    .with_generate(true),
            ),
        }
    }
}

#[async_trait]
impl Provider for StreamingProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    async fn stream(
        &self,
        request: ProviderRequest,
    ) -> Result<ProviderEventStream, ProviderError> {
        let session_id = request.session.id.clone();
        let provider_id = self.descriptor.id.clone();
        let final_message =
            Message::assistant("streamed: subscribe() observed every delta");

        let mut start_meta =
            EventMetadata::new(1, 1).with_provider_id(provider_id.clone());
        let mut update_one_meta =
            EventMetadata::new(2, 2).with_provider_id(provider_id.clone());
        let mut update_two_meta =
            EventMetadata::new(3, 3).with_provider_id(provider_id.clone());
        let mut end_meta = EventMetadata::new(4, 4).with_provider_id(provider_id.clone());
        let mut turn_end_meta = EventMetadata::new(5, 5).with_provider_id(provider_id);

        if let Some(session_id) = &session_id {
            start_meta = start_meta.with_session_id(session_id.clone());
            update_one_meta = update_one_meta.with_session_id(session_id.clone());
            update_two_meta = update_two_meta.with_session_id(session_id.clone());
            end_meta = end_meta.with_session_id(session_id.clone());
            turn_end_meta = turn_end_meta.with_session_id(session_id.clone());
        }

        start_meta = start_meta.with_turn_id(request.turn.id.clone());
        update_one_meta = update_one_meta.with_turn_id(request.turn.id.clone());
        update_two_meta = update_two_meta.with_turn_id(request.turn.id.clone());
        end_meta = end_meta.with_turn_id(request.turn.id.clone());
        turn_end_meta = turn_end_meta.with_turn_id(request.turn.id);

        Ok(Box::pin(stream::iter(vec![
            Ok(AgentEvent::MessageStart {
                meta: start_meta,
                message: Message::assistant(""),
            }),
            Ok(AgentEvent::MessageUpdate {
                meta: update_one_meta,
                message: Message::assistant("streamed: "),
                delta: StreamDelta::text("streamed: "),
            }),
            Ok(AgentEvent::MessageUpdate {
                meta: update_two_meta,
                message: final_message.clone(),
                delta: StreamDelta::text("subscribe() observed every delta"),
            }),
            Ok(AgentEvent::MessageEnd {
                meta: end_meta,
                message: final_message.clone(),
            }),
            Ok(AgentEvent::TurnEnd {
                meta: turn_end_meta,
                message: final_message,
                tool_results: Vec::new(),
            }),
        ])))
    }
}

#[tokio::main]
async fn main() -> Result<(), ExampleError> {
    let agent = Agent::builder()
        .provider(StreamingProvider::new())
        .model("demo-model")
        .build()?;
    let mut subscription = agent.subscribe();
    let agent_for_prompt = agent.clone();

    let prompt_task = tokio::spawn(async move {
        agent_for_prompt
            .prompt("Show me the runtime event stream.")
            .await
    });

    loop {
        let event = subscription.recv().await?;
        println!("event -> {}", describe_event(&event));

        if let AgentEvent::MessageUpdate {
            delta: StreamDelta::Text { text },
            ..
        } = &event
        {
            print!("{text}");
        }

        if matches!(event, AgentEvent::AgentEnd { .. }) {
            break;
        }
    }

    let response = prompt_task.await??;
    println!(
        "\nfinal assistant: {}",
        text_from_message(&response.message)
    );

    Ok(())
}
