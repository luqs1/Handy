---
name: sync-upstream
description: Translate upstream Swift (macOS OpenOats) changes into the cross-platform Tauri/Rust/React desktop app
---

# Sync Upstream Swift → Tauri

You are translating changes from the upstream macOS Swift app (`OpenOats/Sources/OpenOats/`) into the cross-platform Tauri desktop app (`openoats-desktop/`).

## Step 1: Identify What Changed

Run:
```bash
cd /home/luqs1/repos/OpenOats
git log --oneline HEAD...$(cat openoats-desktop/SYNC.md | grep "Last synced commit" | awk '{print $NF}') -- OpenOats/Sources/
```

If there are no changes since last sync, tell the user and stop.

## Step 2: Read the Translation Contract

Read `openoats-desktop/SYNC.md` to understand:
- The module mapping (which Swift file → which Rust/TS file)
- The translation rules (Swift constructs → Rust/TS equivalents)
- The platform API mapping

## Step 3: Check for Platform Divergences

Before translating, check if any changed Swift file touches a platform-specific area.

Read `platform-divergences/README.md` in this skill folder. It has:
- An index mapping Swift files to their divergence docs
- A decision tree for whether to translate, adapt, or skip

If a changed file is covered by a divergence doc, **read that doc first**. It explains:
- What the macOS approach does and why
- What the Windows equivalent is (or should be)
- What to translate vs. what to replace vs. what to skip
- Known limitations on Windows

## Step 4: For Each Changed Swift File

1. **Read the Swift file** and its diff
2. **Find the mapped target** in SYNC.md
3. **If a divergence doc applies**: follow its translation guidance instead of translating literally
4. **If the target exists**: read it, then apply the translated changes
5. **If no target exists**: create a new file at the mapped location

### Translation Guidelines

**Rust (backend logic, domain types, engines):**
- Use `#[derive(Clone, Debug, Serialize, Deserialize)]` on all structs/enums
- Add `#[derive(specta::Type)]` if the type crosses the Tauri command boundary
- Use `serde(rename_all = "camelCase")` for JSON compatibility with the frontend
- Use `chrono::DateTime<Utc>` or `i64` for timestamps
- Use `String` for UUIDs (simpler, JSON-friendly)
- Use `anyhow::Result` for error handling
- Swift `@Observable` classes become plain Rust structs managed via Tauri state
- Swift `actor` becomes `Arc<tokio::sync::Mutex<T>>`
- Swift `Task { }` becomes `tokio::spawn()`
- Keep the same function signatures where possible

**TypeScript/React (UI):**
- Follow existing patterns in `src/components/`
- Use Zustand stores for state that was `@Observable` in Swift
- Use `useTranslation()` for all user-facing strings
- Use Tailwind CSS (match existing component style)
- Invoke Tauri commands via the bindings in `src/bindings.ts`

**When a Swift API has no cross-platform equivalent:**
- Check `platform-divergences/` for guidance on the Windows approach
- If a divergence doc exists: follow its recommended approach
- If no divergence doc exists: create a trait with the same API surface, add `todo!()`, note in SYNC.md as TODO, and create a new divergence doc explaining the gap

## Step 5: Update SYNC.md

- Update the "Last synced commit" to the current HEAD
- Update the Status column for any newly synced files
- Add any new files to the mapping table

## Step 6: Verify

Run:
```bash
cd openoats-desktop
cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | head -50
```

If there are compilation errors, fix them. The goal is a compiling codebase, not necessarily a fully functional one — `todo!()` is acceptable for platform-specific stubs.

## Important Notes

- **Don't translate macOS-only UI code** (SwiftUI views) unless there's a clear React equivalent needed
- **Don't translate Sparkle/LaunchAtLogin** — Tauri has its own plugins for these
- **Preserve the existing Tauri patterns** — don't restructure the desktop app to match Swift's patterns exactly. Adapt the logic to fit the existing manager/command architecture.
- **Domain types should be as close to 1:1 as possible** — these are the "contract" between the two codebases
