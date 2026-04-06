use std::convert::Infallible;

use axum::body::Body;
use bytes::Bytes;
use futures::{StreamExt, stream};

use crate::domain::response::{EventStream, StreamEvent};

pub fn encode_event(model: &str, event: StreamEvent) -> Vec<String> {
    match event {
        StreamEvent::Started => vec![],
        StreamEvent::TextDelta(text) => {
            let payload = serde_json::json!({
                "object": "chat.completion.chunk",
                "model": model,
                "choices": [
                    {
                        "index": 0,
                        "delta": { "content": text },
                        "finish_reason": serde_json::Value::Null
                    }
                ]
            });
            vec![format!("data: {}\n\n", payload)]
        }
        StreamEvent::Usage(_) => vec![],
        StreamEvent::Completed => vec!["data: [DONE]\n\n".into()],
        StreamEvent::Error(message) => {
            let payload = serde_json::json!({
                "error": { "message": message, "type": "server_error" }
            });
            vec![format!("data: {}\n\n", payload), "data: [DONE]\n\n".into()]
        }
    }
}

pub async fn collect_sse_chunks(model: &str, stream: EventStream) -> Vec<String> {
    stream
        .filter_map(|event| async move { event.ok() })
        .flat_map(move |event| stream::iter(encode_event(model, event)))
        .collect()
        .await
}

pub fn sse_body(model: String, stream: EventStream) -> Body {
    let body_stream = stream.flat_map(move |result| {
        let chunks = match result {
            Ok(event) => encode_event(&model, event),
            Err(error) => encode_event(&model, StreamEvent::Error(error.to_string())),
        };

        stream::iter(
            chunks
                .into_iter()
                .map(|chunk| Ok::<Bytes, Infallible>(Bytes::from(chunk))),
        )
    });

    Body::from_stream(body_stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::response::{EventStream, StreamEvent};

    #[tokio::test]
    async fn encodes_live_stream_events_into_sse_chunks() {
        let stream: EventStream = Box::pin(stream::iter(vec![
            Ok(StreamEvent::TextDelta("hel".into())),
            Ok(StreamEvent::TextDelta("lo".into())),
            Ok(StreamEvent::Completed),
        ]));

        let chunks = collect_sse_chunks("gpt-4.1", stream).await;
        assert!(chunks[0].contains("\"delta\""));
        assert_eq!(chunks.last().unwrap(), "data: [DONE]\n\n");
    }
}
