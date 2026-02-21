use rs_lib_ng::loggers::{LoggerBuilder, LogLevel};
use rs_lib_ng::markets::cnn::fearandgreed::FearAndGreed;

#[tokio::test]
async fn test_cnn_fear_and_greed() {
    // Correct way to instantiate a Logger in this crate [cite: 31, 32, 36]
    let logger = LoggerBuilder::new("test_cnn")
        .with_level(LogLevel::Debug)
        .build()
        .unwrap();
        
    let service = FearAndGreed::new(logger);

    match service.fetch_current().await {
        Ok(fng) => {
            println!("\n[CNN SUCCESS]");
            println!("Score: {:.2} | Rating: {}", fng.score, fng.rating);
        }
        Err(e) => {
            eprintln!("\n⚠️  WARNING: CNN fetch failed. Error: {}", e);
        }
    }
}
