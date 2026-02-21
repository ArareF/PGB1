# PGB1 Phase 7 Implementation Plan: Global Shortcuts & Backend Services

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement global shortcuts functionality using `tauri-plugin-global-shortcut` and backend storage for user-defined shortcuts.

**Architecture:**
- **Backend (Rust)**:
  - `shortcuts` module handles storage (JSON file) and business logic.
  - `global_shortcut` plugin handles OS-level key interception.
  - `extract_icon` command uses Windows API to extract icons from .exe files.
- **Frontend (Vue)**:
  - `Sidebar.vue` integrates drag-and-drop sortable list.
  - `ShortcutDialog.vue` handles adding/editing shortcuts (App/Folder/Web).

**Tech Stack:** Tauri 2.0, Rust (serde, windows-rs), Vue 3, SortableJS (optional or native drag).

---

## Task 1: Backend - Shortcuts Module & Storage

**Files:**
- Create: `src-tauri/src/shortcuts.rs`
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: Define Shortcut Model**

In `src-tauri/src/models.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShortcutType {
    App,    // .exe
    Folder, // Directory
    Web,    // URL
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shortcut {
    pub id: String,
    pub name: String,
    pub shortcut_type: ShortcutType,
    pub path: String, // Path or URL
    pub icon: String, // Path to cached icon or URL/favicon
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShortcutConfig {
    pub shortcuts: Vec<Shortcut>,
}
```

**Step 2: Implement Storage Logic**

In `src-tauri/src/shortcuts.rs`:

```rust
use crate::models::{Shortcut, ShortcutConfig};
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;

pub fn load_shortcuts(app: &AppHandle) -> Result<Vec<Shortcut>, String> {
    // Logic to read shortcuts.json from AppData
    // Return empty list if file not found
}

pub fn save_shortcuts(app: &AppHandle, shortcuts: Vec<Shortcut>) -> Result<(), String> {
    // Logic to write shortcuts.json
}
```

**Step 3: Register Commands**

In `src-tauri/src/commands.rs` (create if needed or add to `lib.rs` commands):

- `load_shortcuts`
- `save_shortcuts`

---

## Task 2: Backend - Icon Extraction (Windows)

**Files:**
- Modify: `src-tauri/Cargo.toml` (add `image` feature for icon processing if needed)
- Modify: `src-tauri/src/shortcuts.rs`

**Step 1: Implement `extract_icon`**

Using `windows` crate (already in dependencies):
- `PrivateExtractIconsW` or `ExtractIconExW` to get HICON.
- Convert HICON to bitmap/png bytes.
- Save to `AppData/icons/<uuid>.png`.
- Return the local path string.

**Step 2: Register Command**

- `extract_icon(path: String) -> Result<String, String>`

---

## Task 3: Frontend - Store & Dialog

**Files:**
- Create: `src/composables/useShortcuts.ts`
- Create: `src/components/ShortcutDialog.vue`

**Step 1: `useShortcuts.ts`**

- State: `shortcuts` array.
- Actions: `load`, `add`, `remove`, `update`, `reorder`.
- Call backend commands.

**Step 2: `ShortcutDialog.vue`**

- Form with: Type (App/Folder/Web), Path/URL, Name.
- "Browse" button for App/Folder (invokes `open` dialog).
- Auto-fill Name and Icon logic:
  - If App selected: call `extract_icon` + use filename.
  - If Web: use default globe icon + input URL.
  - If Folder: use default folder icon + folder name.

---

## Task 4: Frontend - Sidebar Integration

**Files:**
- Modify: `src/components/Sidebar.vue`

**Step 1: Render List**

- Loop through `shortcuts`.
- Display Icon + Name (tooltip?).
- Style as a vertical list.

**Step 2: Drag & Drop Sorting**

- Use HTML5 Drag & Drop API or `vuedraggable` (if added).
- On drop, update list order and call `save_shortcuts`.

**Step 3: Context Menu**

- Right-click: Edit / Delete.
- Invoke `ShortcutDialog` for editing.

---

## Task 5: Global Shortcut (Translation Feature)

**Files:**
- Modify: `src-tauri/Cargo.toml` (add `tauri-plugin-global-shortcut`)
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/capabilities/default.json`

**Step 1: Add Plugin**

```toml
[dependencies]
tauri-plugin-global-shortcut = "2.0.0-rc"
```

**Step 2: Register & Configure**

In `lib.rs`:
- `.plugin(tauri_plugin_global_shortcut::Builder::new().build())`
- On app setup: register the shortcut defined in settings (e.g., `Ctrl+Shift+T`).
- Handler: verify shortcut matches, then emit event or show window directly.

**Step 3: Calculator Key Interception**

- Special handling for `VK_LAUNCH_APP2` (Calculator) if possible via `global-shortcut` or low-level hook (might be complex, start with standard keys).

---

## Execution Handoff

Plan complete and saved to `docs/plans/2026-02-18-phase7-shortcuts.md`.

**Two execution options:**

1.  **Subagent-Driven (this session)** - I dispatch fresh subagent per task.
2.  **Parallel Session (separate)** - Open new session with executing-plans.

**Which approach?**
