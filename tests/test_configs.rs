use rs_lib_ng::configs::ConfigManager;
use std::env;

#[tokio::test]
async fn test_config_choices() {
    // 1. Test Local Choice (will fail if file doesn't exist)
    let local_res = ConfigManager::get_local_config("config.json");
    match local_res {
        Ok(mgr) => println!("Local Loaded: {:?}", mgr.get()),
        Err(e) => println!("Local Load skipped/failed (expected if no file): {}", e),
    }

    // 2. Test Cloud Choice (requires WEBLIB_AES_PASSWORD)
    if let Ok(url) = env::var("WEBLIB_CLOUD_CONFIG_URL") {
        let cloud_res = ConfigManager::get_cloud_config(&url).await;
        match cloud_res {
            Ok(mgr) => {
                let data = mgr.get();
                assert!(data.is_object());
                println!("Cloud Loaded: {}", serde_json::to_string_pretty(&*data).unwrap());
            },
            Err(e) => panic!("Cloud Config strictly failed: {}", e),
        }
    }
}
