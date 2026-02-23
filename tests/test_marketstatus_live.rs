//! # Nasdaq Market Status Live Integration Test
//!
//! This test performs an actual network call to the live Nasdaq API.
//! It is used to verify that the API structure hasn't changed and that 
//! our browser-mimicry headers are still effective.

use rs_lib_ng::markets::nasdaq::marketstatus::MarketStatus;
use rs_lib_ng::loggers::builder::LoggerBuilder;

#[tokio::test]
async fn test_market_status_live_call() {
    //! Scenario: Real network call to Nasdaq.
    //! Goal: Print the actual live response and verify parsing.
    
    // 1. Initialize a real logger to see the request/retry telemetry
    let logger = LoggerBuilder::new("nasdaq_live_test")
        .build()
        .expect("Failed to initialize logger");

    // 2. Initialize the MarketStatus service
    let service = MarketStatus::new(logger);

    // 3. Perform the live call
    // We use fetch_raw first to see exactly what the server sends
    println!("--- FETCHING RAW LIVE DATA ---");
    let raw_result = service.fetch_raw(None).await;
    
    match raw_result {
        Ok(json) => {
            // Print the pretty-printed JSON to the test console
            println!("{}", serde_json::to_string_pretty(&json).unwrap());
            
            // Verify that the 'status' and 'data' blocks exist
            assert!(json.get("data").is_some(), "Live response missing 'data' field");
            assert_eq!(json["status"]["rCode"], 200, "Nasdaq returned a business error");
        },
        Err(e) => panic!("Live API call failed: {:?}", e),
    }

    // 4. Verify that our typed mapping works with the live data
    println!("--- VERIFYING TYPED DESERIALIZATION ---");
    let status_result = service.fetch_status(None).await;
    
    match status_result {
        Ok(data) => {
            println!("Successfully parsed live data!");
            println!("Country: {}", data.country);
            println!("Current Status: {}", data.mrkt_status);
            println!("Next Trade Date: {}", data.next_trade_date);
            
            // Check session logic against live time
            let is_open = service.is_regular_session(&data);
            println!("Is Regular Session (Live): {}", is_open);
            
            // Check delay calculation
            if let Ok(delay) = service.get_next_opening_delay(&data) {
                println!("Time until next open: {}", service.format_duration(
                    chrono::Duration::from_std(delay).unwrap()
                ));
            }
        },
        Err(e) => panic!("Failed to map live JSON to struct: {:?}", e),
    }
}
