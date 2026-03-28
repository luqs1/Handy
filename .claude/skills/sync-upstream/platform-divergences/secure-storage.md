# Secure Storage (API Keys & Secrets)

## What it does
Stores sensitive credentials (API keys for OpenRouter, Voyage AI, etc.) securely outside of plain-text config files.

## macOS Approach
**API:** macOS Keychain (`Security.framework`)

The Swift app stores API keys in the system Keychain via `SecItemAdd`/`SecItemCopyMatching`. Keys are encrypted at rest, protected by the user's login password, and sandboxed per-app.

Lives in: `Settings/SettingsStorage.swift` (Keychain read/write helpers)

## Windows Approach
**API:** Windows Credential Manager or DPAPI

The desktop app currently stores API keys in `tauri-plugin-store` (a JSON file in app data). This is **not secure** — keys are in plaintext.

### Recommended upgrade path
- **Windows Credential Manager** via the `keyring` crate — stores secrets in the Windows Credential Vault, encrypted by the user's login
- **DPAPI** via the `windows` crate — encrypts data tied to the current user account

```rust
// Using the keyring crate (cross-platform)
let entry = keyring::Entry::new("openoats", "openrouter-api-key")?;
entry.set_password("sk-...")?;
let key = entry.get_password()?;
```

The `keyring` crate works on macOS (Keychain), Windows (Credential Manager), and Linux (Secret Service/kwallet) — making it the best cross-platform choice.

## Divergence Summary

| Aspect | macOS | Windows |
|---|---|---|
| Backend | Keychain (Security.framework) | Credential Manager (or DPAPI) |
| Encryption | AES-256, tied to login keychain | DPAPI, tied to user account |
| Cross-platform crate | `keyring` | `keyring` |

## Translation Guidance

When the Swift code changes Keychain-related code in `SettingsStorage.swift`:
- **Translate** the key names and storage/retrieval logic
- **Don't translate** `SecItem*` API calls — use the `keyring` crate instead
- The concept is identical: store(service, key, value) / retrieve(service, key)

## Implementation Priority
Low — the app works with plaintext storage. But should be done before any public release for security.
