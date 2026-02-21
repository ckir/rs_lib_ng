use aes::Aes256;
use base64::{Engine as _, engine::general_purpose};
use cbc::Decryptor;
use cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};
use serde_json::Value;
use std::env;
use crate::core::error::NgError;

pub async fn load_remote_json(url: &str) -> Result<Value, NgError> {
    let password = env::var("WEBLIB_AES_PASSWORD")
        .map_err(|_| NgError::ConfigError("Missing WEBLIB_AES_PASSWORD".into()))?;

    let client = reqwest::Client::new();
    let response = client.get(url).send().await
        .map_err(|e| NgError::ConfigError(format!("Network Error: {}", e)))?;

    let content = response.text().await
        .map_err(|e| NgError::ConfigError(format!("Read Error: {}", e)))?;

    let lines: Vec<&str> = content.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    if lines.len() < 2 {
        return Err(NgError::ConfigError("Invalid S3 file format: expected IV and Ciphertext".into()));
    }

    let iv = general_purpose::STANDARD.decode(lines[0]).map_err(|_| NgError::ConfigError("Invalid IV".into()))?;
    let ciphertext = general_purpose::STANDARD.decode(lines[1]).map_err(|_| NgError::ConfigError("Invalid Ciphertext".into()))?;
    let key_vec = hex::decode(password.trim()).map_err(|_| NgError::ConfigError("Invalid Key Hex".into()))?;

    let decryptor = Decryptor::<Aes256>::new((&key_vec[..32]).into(), (&iv[..16]).into());
    let mut buf = ciphertext.to_vec();
    let decrypted_data = decryptor.decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|e| NgError::ConfigError(format!("Decryption failed: {:?}", e)))?;

    serde_json::from_slice(decrypted_data).map_err(|e| NgError::ConfigError(e.to_string()))
}
