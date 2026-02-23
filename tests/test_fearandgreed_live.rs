//! # Fear & Greed Live Integration Test
//!
//! Performs real network requests to CNN Business to ensure headers
//! and data mapping work against the live production environment.

use rs_lib_ng::markets::cnn::fearandgreed::FearAndGreed;
use rs_lib_ng::loggers::builder::LoggerBuilder;

#[tokio::test]
#[ignore] // Run with: cargo test --test test_fearandgreed_live -- --ignored --nocapture
async fn test_fng_live_retrieval() {
    let logger = LoggerBuilder::new("fng_live_test").build().unwrap();
    let service = FearAndGreed::new(logger);

    println!("--- FETCHING LATEST LIVE DATA ---");
    let result = service.fetch_latest(None).await;

    match result {
        Ok(status) => {
            println!("Success! Current Score: {} ({})", status.current.value, status.current.rating);
            println!("Timestamp: {}", status.current.date);
            
            // Verify we got historical data points
            assert!(!status.history.is_empty(), "History should not be empty");
            println!("Historical points retrieved: {}", status.history.len());

            // Verify a sub-indicator
            println!("Market Momentum: {} ({})", 
                status.market_momentum.value, 
                status.market_momentum.rating
            );
        },
        Err(e) => panic!("Live fetch failed: {:?}", e),
    }
}

#[tokio::test]
#[ignore]
async fn test_fng_historical_live() {
    let logger = LoggerBuilder::new("fng_live_hist").build().unwrap();
    let service = FearAndGreed::new(logger);
    
    // Test a specific recent date
    let target_date = "2024-02-20";
    let result = service.fetch_at_date(target_date, None).await;

    assert!(result.is_ok(), "Failed to fetch historical date: {:?}", result.err());
    let data = result.unwrap();
    println!("Historical date {} value: {}", target_date, data.current.value);
}
