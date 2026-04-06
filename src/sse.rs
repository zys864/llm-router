use crate::domain::response::StreamEvent;

pub fn encode_events(model: &str, events: Vec<StreamEvent>) -> Vec<String> {
    let mut chunks = Vec::new();

    for event in events {
        match event {
            StreamEvent::Started => {}
            StreamEvent::DeltaText(text) => {
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
                chunks.push(format!("data: {}\n\n", payload));
            }
            StreamEvent::Usage(_) => {}
            StreamEvent::Completed => chunks.push("data: [DONE]\n\n".into()),
            StreamEvent::Error(message) => {
                let payload = serde_json::json!({
                    "error": { "message": message, "type": "server_error" }
                });
                chunks.push(format!("data: {}\n\n", payload));
                chunks.push("data: [DONE]\n\n".into());
            }
        }
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::response::StreamEvent;

    #[test]
    fn encodes_delta_and_done_events() {
        let chunks = encode_events(
            "gpt-4.1",
            vec![
                StreamEvent::DeltaText("hel".into()),
                StreamEvent::DeltaText("lo".into()),
                StreamEvent::Completed,
            ],
        );

        assert!(chunks[0].contains("\"delta\""));
        assert_eq!(chunks.last().unwrap(), "data: [DONE]\n\n");
    }
}
