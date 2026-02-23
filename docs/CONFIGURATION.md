# Configuration

## Local config
Use `ConfigManager::get_local_config(path)` to load a local JSON file and merge environment variables prefixed with `WEBLIB_`. The manager stores the merged config in an `ArcSwap` for lock-free reads.

## Cloud config
`ConfigManager::get_cloud_config(url)` downloads an encrypted JSON blob, decrypts it using `configs::cloud::load_remote_json`, and merges `commonAll` with a binary-specific section (binary name derived from `current_exe()`).

**Environment variables**
- `WEBLIB_AES_PASSWORD` — required for decrypting cloud config files.
