use futures_util::{StreamExt, SinkExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use crate::loggers::Logger;
use crate::core::error::NgError;
use crate::error;
use serde_json::json;
use base64::{Engine as _, engine::general_purpose};

pub struct YahooStreaming {
    uri: String,
    logger: Logger,
}

impl YahooStreaming {
    pub fn new(logger: Logger) -> Self {
        Self {
            uri: "wss://streamer.finance.yahoo.com/?version=2".to_string(),
            logger,
        }
    }

    /// Connects to the stream and processes incoming price data
    pub async fn stream_quotes(&self, symbols: Vec<&str>) -> Result<(), NgError> {
        let (mut ws_stream, _) = connect_async(&self.uri).await
            .map_err(|e| NgError::InternalError(format!("WS Connection Failed: {}", e)))?;

        // Yahoo requires a JSON subscription message
        let subscribe_msg = json!({ "subscribe": symbols }).to_string();
        ws_stream.send(Message::Text(subscribe_msg)).await
            .map_err(|e| NgError::InternalError(format!("Failed to send subscription: {}", e)))?;

        println!("📡 Yahoo WebSocket active for: {:?}", symbols);

        while let Some(msg) = ws_stream.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // 1. Decode Base64
                    if let Ok(bin_data) = general_purpose::STANDARD.decode(&text) {
                        // 2. Map to Protobuf (PricingData)
                        // This assumes you have the generated prost code in your crate
                        self.handle_proto_data(bin_data);
                    }
                }
                Ok(Message::Close(frame)) => {
                    println!("🚪 Connection closed by server: {:?}", frame);
                    break;
                }
                Err(e) => {
                    error!(self.logger, "WS Stream Error", "error" => e.to_string());
                    return Err(NgError::InternalError(e.to_string()));
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn handle_proto_data(&self, _data: Vec<u8>) {
        // Here you would call: PricingData::decode(&data[..])
        // For now, we log the receipt of binary packets
        println!("📦 Received binary update ({} bytes)", _data.len());
    }
}
