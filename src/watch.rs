use anyhow::{Context, Result, bail};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::cli::WatchClobArgs;

pub async fn watch_clob(args: WatchClobArgs, json_output: bool) -> Result<()> {
    validate_args(&args)?;
    let assets = load_assets(&args)?;
    if assets.is_empty() {
        bail!("provide at least one --asset or --assets-file");
    }

    let (mut socket, _) = connect_async(&args.ws_url)
        .await
        .with_context(|| format!("failed to connect {}", args.ws_url))?;
    for chunk in assets.chunks(args.chunk_size) {
        socket
            .send(Message::Text(
                json!({
                    "assets_ids": chunk,
                    "type": "market",
                    "custom_feature_enabled": true
                })
                .to_string()
                .into(),
            ))
            .await?;
    }

    let mut ping = tokio::time::interval(std::time::Duration::from_secs(args.ping_secs));
    loop {
        tokio::select! {
            _ = ping.tick() => {
                socket.send(Message::Text("PING".into())).await?;
            }
            message = socket.next() => {
                let Some(message) = message else { break };
                let message = message?;
                if let Some(text) = payload_text(message)? {
                    print_payload(&text, json_output)?;
                    if args.once {
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_args(args: &WatchClobArgs) -> Result<()> {
    if args.chunk_size == 0 {
        bail!("--chunk-size must be greater than zero");
    }
    if args.ping_secs == 0 {
        bail!("--ping-secs must be greater than zero");
    }
    Ok(())
}

fn load_assets(args: &WatchClobArgs) -> Result<Vec<String>> {
    let mut assets = args.assets.clone();
    if let Some(path) = args.assets_file.as_ref() {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        assets.extend(
            text.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(ToOwned::to_owned),
        );
    }
    assets.sort();
    assets.dedup();
    Ok(assets)
}

fn payload_text(message: Message) -> Result<Option<String>> {
    match message {
        Message::Text(text) => {
            let text = text.to_string();
            Ok((text != "PONG").then_some(text))
        }
        Message::Binary(bytes) => Ok(Some(String::from_utf8(bytes.to_vec())?)),
        Message::Ping(_) | Message::Pong(_) | Message::Close(_) | Message::Frame(_) => Ok(None),
    }
}

fn print_payload(payload: &str, json_output: bool) -> Result<()> {
    let value =
        serde_json::from_str::<Value>(payload).unwrap_or_else(|_| Value::String(payload.into()));
    if json_output {
        println!("{}", serde_json::to_string(&value)?);
    } else {
        match value {
            Value::Array(items) => {
                for item in items {
                    print_human_event(&item);
                }
            }
            other => print_human_event(&other),
        }
    }
    Ok(())
}

fn print_human_event(value: &Value) {
    let event_type = value
        .get("event_type")
        .or_else(|| value.get("type"))
        .and_then(Value::as_str)
        .unwrap_or("event");
    let asset = value
        .get("asset_id")
        .or_else(|| value.get("asset"))
        .or_else(|| value.get("token_id"))
        .and_then(Value::as_str)
        .unwrap_or("-");
    println!("{event_type} asset={asset}");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_args() -> WatchClobArgs {
        WatchClobArgs {
            ws_url: "wss://example.invalid/ws".to_string(),
            assets: vec!["123".to_string()],
            assets_file: None,
            chunk_size: 500,
            ping_secs: 10,
            once: true,
        }
    }

    #[test]
    fn watch_clob_rejects_zero_intervals() {
        let mut args = base_args();
        args.ping_secs = 0;

        assert!(validate_args(&args).is_err());

        let mut args = base_args();
        args.chunk_size = 0;

        assert!(validate_args(&args).is_err());
    }
}
