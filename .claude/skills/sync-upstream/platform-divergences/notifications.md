# Notifications

## What it does
Shows system notifications when meetings are detected, recordings start/stop, or errors occur.

## macOS Approach
**API:** `UserNotifications.framework` (`UNUserNotificationCenter`)

The Swift app requests notification permission and posts local notifications with actions (e.g., "Start Recording", "Dismiss"). Uses `UNNotificationAction` for interactive buttons.

Lives in: `Meeting/NotificationService.swift`

## Windows Approach
**API:** `tauri-plugin-notification` (already available) or Win32 Toast notifications

The Tauri app can use `tauri-plugin-notification` which wraps platform-native notifications:
- Windows: Toast notifications via WinRT
- Linux: libnotify / D-Bus notifications

Interactive actions (buttons on notifications) are supported via Tauri's notification plugin with `addAction()`.

## Divergence Summary

| Aspect | macOS | Windows |
|---|---|---|
| API | UNUserNotificationCenter | WinRT Toast (via tauri-plugin-notification) |
| Permission | Must request via system prompt | No permission needed on Windows |
| Actions | UNNotificationAction | Toast buttons via Tauri plugin |

## Translation Guidance

When the Swift code changes `NotificationService.swift`:
- **Translate** the notification content (title, body, action labels)
- **Translate** the notification scheduling logic
- **Don't translate** `UNUserNotificationCenter` setup — use Tauri's notification plugin
- **Don't translate** permission requests — Windows doesn't need them

## Implementation Priority
Low. Nice-to-have for meeting detection. Not needed until auto-detection is implemented.
