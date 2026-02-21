use rs_lib_ng::loggers::{LoggerBuilder, LogLevel};
use rs_lib_ng::markets::nasdaq::marketstatus::MarketStatus;

#[tokio::test]
async fn test_full_market_data_output() {
    let logger = LoggerBuilder::new("test_bin").with_level(LogLevel::Debug).build().unwrap();
    let service = MarketStatus::new(logger);

    match service.fetch_raw().await {
        Ok(json) => {
            println!("\n[FULL RAW DATA RECEIVED]\n{}", serde_json::to_string_pretty(&json).unwrap());
            let status = service.fetch_status().await.expect("Failed to parse struct");
            println!("\n[PARSED STATUS] Indicator: {}", status.market_indicator);
            assert!(!status.market_indicator.is_empty());
        }
        Err(e) => panic!("Market fetch failed: {}", e),
    }
}
