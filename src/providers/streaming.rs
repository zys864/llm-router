use bytes::Bytes;
use futures::{Stream, StreamExt, stream};
use serde_json::Value;

use crate::domain::response::{EventStream, StreamEvent};

pub fn sse_json_stream<S, F>(stream: S, map: F) -> EventStream
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
    F: Fn(Value) -> Option<StreamEvent> + Send + Sync + 'static + Copy,
{
    let stream = stream
        .map(|chunk| chunk.map_err(super::map_reqwest_error))
        .flat_map(move |chunk| match chunk {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes).to_string();
                let mut events = Vec::new();
                for frame in text.split("\n\n") {
                    if let Some(payload) = frame.strip_prefix("data: ") {
                        if payload == "[DONE]" {
                            events.push(Ok(StreamEvent::Completed));
                        } else if let Ok(json) = serde_json::from_str::<Value>(payload) {
                            if let Some(event) = map(json) {
                                events.push(Ok(event));
                            }
                        }
                    }
                }
                stream::iter(events)
            }
            Err(error) => stream::iter(vec![Err(error)]),
        });

    Box::pin(stream)
}
