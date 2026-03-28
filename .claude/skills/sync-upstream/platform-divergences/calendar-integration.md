# Calendar Integration

## What it does
Reads upcoming calendar events to detect scheduled meetings and pre-populate meeting metadata (title, participants, organizer).

## macOS Approach
**API:** EventKit (`EKEventStore`)

The Swift app requests calendar access, queries for events in a time window, and checks if any are online meetings (presence of a meeting URL, video call link, etc.). Calendar events feed into the `DetectionContext` as a `DetectionSignal.calendarEvent`.

Lives in: referenced in `Domain/MeetingTypes.swift` (CalendarEvent type), used by `App/MeetingDetectionController.swift`

## Windows Approach
**No direct equivalent API.** Options:

### Option 1: Outlook COM Interop (Windows-only)
- Use the `windows` crate to access Outlook's COM API
- Can read calendar items from the default Outlook profile
- Only works if the user has Outlook installed
- Heavy dependency, fragile

### Option 2: ICS/CalDAV fetch
- Let the user provide a CalDAV URL or .ics subscription link
- Parse with the `icalendar` crate
- Works with Google Calendar, Outlook 365, iCloud, etc.
- More universal but requires user configuration

### Option 3: Google Calendar / Microsoft Graph API
- OAuth2 flow to connect to cloud calendars
- Most flexible but requires API keys and auth infrastructure
- The desktop app already has LLM API key infrastructure that could be extended

### Option 4: Skip it
- Calendar integration is a nice-to-have for auto-detection
- Manual start/stop works fine
- Defer until core features are solid

## Divergence Summary

| Aspect | macOS | Windows |
|---|---|---|
| API | EventKit (system-level, all calendars) | No equivalent — need Outlook COM, CalDAV, or cloud API |
| Auth | One-time system permission prompt | Varies: COM needs Outlook installed, CalDAV/API needs user config |
| Scope | All calendars on the Mac | Depends on approach chosen |

## Translation Guidance

When the Swift code changes calendar-related code:
- **Translate** the `CalendarEvent` struct and how it feeds into detection — these are platform-independent domain types (already in `domain/meeting_types.rs`)
- **Don't translate** EventKit API calls — there's no equivalent
- **Translate** the logic that decides "this calendar event is an online meeting" (URL pattern matching, time window checks) — this is reusable regardless of how we source the calendar data

## Implementation Priority
Low. Calendar integration is a detection enhancement, not core functionality. Recommend Option 4 (skip) for MVP, Option 2 (ICS/CalDAV) for v2.
