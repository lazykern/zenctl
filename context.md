# Zen Browser Source Code Map — Features 9-19

## Files Retrieved

### 9. Boosts
1. `zen-browser/src/zen/boosts/ZenBoostsManager.sys.mjs` (lines 1-550) — Core manager: CRUD, persistence, styling
2. `zen-browser/src/zen/boosts/ZenBoostsEditor.mjs` (lines 1-1637) — In-page editor UI + code CSS editor
3. `zen-browser/src/zen/boosts/ZenBoostStyles.sys.mjs` (lines 1-120) — CSS generation + agent sheet cache
4. `zen-browser/src/zen/boosts/ZenSelectorComponent.sys.mjs` (lines 1-570) — CSS selector picker overlay
5. `zen-browser/src/zen/boosts/ZenZapDissolve.sys.mjs` (lines 1-470) — WebGL dissolve effect
6. `zen-browser/src/zen/boosts/ZenZapOverlayChild.sys.mjs` (lines 1-470) — Zap UI overlay

### 10. Zen Mods
7. `zen-browser/src/zen/mods/ZenMods.mjs` (lines 1-543) — Mod manager: install, enable, update, CSS rebuild

### 11. Theme System
8. `zen-browser/src/zen/common/zenThemeModifier.js` (lines 1-182) — Accent color, border radius, element separation injection

### 12. Keyboard Shortcuts
9. `zen-browser/src/zen/kbs/ZenKeyboardShortcuts.mjs` (lines 1-1030+) — Shortcut manager, migration, conflict detection

### 13. URL Bar
10. `zen-browser/src/zen/urlbar/ZenUBActionsProvider.sys.mjs` (lines 1-422) — Quick actions URL bar provider
11. `zen-browser/src/zen/urlbar/ZenUBProvider.sys.mjs` (lines 1-30) — Registration entry point
12. `zen-browser/src/zen/urlbar/ZenUBGlobalActions.sys.mjs` (lines 1-225) — Global actions template (compact mode, theme, split view, etc.)
13. `zen-browser/src/zen/urlbar/ZenUBResultsLearner.sys.mjs` (lines 1-114) — ML-based result prioritization
14. `zen-browser/src/zen/urlbar/ZenSiteDataPanel.sys.mjs` (lines 1-644) — URL bar site data panel (permissions, boosts, addons)

### 14. Media Controls
15. `zen-browser/src/zen/media/ZenMediaController.mjs` (lines 1-470) — Sidebar media control bar

### 15. Window Sync
16. `zen-browser/src/zen/sessionstore/ZenWindowSync.sys.mjs` (lines 1-1708) — Cross-window tab sync via docshell swap
17. `zen-browser/src/zen/sessionstore/ZenSessionManager.sys.mjs` (lines 1-590) — Custom session store (sidebar persistence)

### 16. Drag-and-Drop
18. `zen-browser/src/zen/drag-and-drop/ZenDragAndDrop.js` (lines 1-1820) — Custom DnD: split-view drop, workspace switch, essentials grid

### 17. Downloads
19. `zen-browser/src/zen/downloads/ZenDownloadAnimation.mjs` (lines 1-380) — Download arc animation + box entry/exit

### 18. Welcome
20. `zen-browser/src/zen/welcome/ZenWelcome.mjs` (lines 1-660) — Multi-stage welcome wizard

### 19. Emoji Picker
21. `zen-browser/src/zen/common/emojis/ZenEmojiPicker.mjs` (lines 1-260) — Emoji/SVG icon picker panel

---

## Key Code

### 9. Boosts (`src/zen/boosts/`)

#### `ZenBoostsManager` — Singleton `gZenBoostsManager`
- **File**: `ZenBoostsManager.sys.mjs`
- **Class**: `nsZenBoostsManager`
- **Storage**: `zen-boosts.jsonlz4` in profile dir. Per-domain boost map + activeBoostId.
- **Public methods**:
  - `getActiveBoostId(domain)` → `string|null`
  - `deleteBoost(boost)` — removes boost, invalidates style, writes disk
  - `getEmptyBoostEntry()` → `{ boostData: {...} }` — template with defaults (brightness, saturation, contrast, fontFamily, customCSS, zapSelectors, etc.)
  - `createNewBoost(domain)` → `{ id, domain, boostEntry }` — UUID id
  - `loadBoostsFromStore(domain)` → `object[]` — all boosts for domain
  - `loadBoostFromStore(domain, id)` → `object` — specific boost, creates if missing
  - `loadActiveBoostFromStore(domain)` → `object|null`
  - `addZapSelectorToActive(selector, domain)` / `removeZapSelectorToActive(selector, domain)`
  - `clearZapSelectorsForActive(domain)`
  - `makeBoostActiveForDomain(domain, id)` / `toggleBoostActiveForDomain(domain, id)`
  - `updateBoost(boost)` — in-memory update, triggers notify
  - `saveBoostToStore(boost)` — persist + notify
  - `registeredBoostForDomain(domain)` → `boolean`
  - `canBoostSite(uri)` → `boolean` — http/https only
  - `getStyleSheetForBoost(domain)` → `nsIStyleSheet` — via `nsZenBoostStyles`
  - `openBoostWindow(parentWindow, boost, domainUri)` → `Window` — opens editor popup
  - `exportBoost(parentWindow, boostData)` → `Promise<boolean>` — file save dialog
  - `importBoost(parentWindow)` → `Promise<object|null>` — file open dialog
- **Notifications**: `zen-boosts-update`, `zen-boosts-active-change`
- **Dependencies**: `JSONFile`, `nsZenBoostStyles`

#### `nsZenBoostStyles`
- **File**: `ZenBoostStyles.sys.mjs`
- **Class**: `nsZenBoostStyles`
- **Methods**:
  - `getStyleForBoost(boostData, domain)` → `string` (CSS)
  - `invalidateStyleForDomain(domain)` — unregisters agent sheet, clears cache
- **Internal**: `#generateStyleString(boostData)` builds CSS: zap blocks, font-family, text-transform, customCSS. Uses `nsIStyleSheetService.AGENT_SHEET`.

#### `nsZenBoostEditor`
- **File**: `ZenBoostsEditor.mjs`
- **Class**: `nsZenBoostEditor`
- **Methods**: color picker (WebGL gradient), font selector, case/size toggles, CSS code editor (CodeMirror via DevToolsLoader), zap mode, selector picker
- **Key actions**: `updateCurrentBoost()`, `saveBoost()`, `deleteBoost()`, `shuffleBoost()`, `resetBoost()`
- **Observers**: `zen-boosts-kill-editor`, `zap-list-update`, `zap-state-update`, `selector-picker-state-update`, `zen-boosts-active-change`

#### `ZapOverlay` + `SelectorComponent` + `ZapDissolve`
- **SelectorComponent** (`ZenSelectorComponent.sys.mjs`): Anonymous content overlay for CSS selector picking. `initialize()`, `handleEvent(event, prevent)`, `getSelectionPath(document, relatedValueIndex, selectedElement)` → CSS selector string. States: SELECTING, SELECTED.
- **ZapOverlay** (`ZenZapOverlayChild.sys.mjs`): Manages zap list UI. `initialize()`, `handleZap(cssPath)`, `handleUnzap(cssPath)`, `handleEvent(event, prevent)`.
- **ZapDissolve** (`ZenZapDissolve.sys.mjs`): WebGL dissolve effect. `initialize()`, `dissolve(element, onComplete)`. Pool of 5 dissolve effects.

---

### 10. Zen Mods (`src/zen/mods/`)

#### `nsZenMods`
- **File**: `ZenMods.mjs` (extends `nsZenPreloadedFeature`)
- **Singleton**: `window.gZenMods`
- **Storage**: `zen-themes.json` in profile, mod files under `chrome/zen-themes/<modId>/`
- **Stylesheet output**: `chrome/zen-themes.css`
- **Public methods**:
  - `init()` — reads mods, rebuilds stylesheet, sets pref defaults, auto-updates via `checkForModsUpdates()`
  - `getMods()` → `object` — reads `zen-themes.json`
  - `getModFolder(modId)` → path
  - `getModPreferences(mod)` → `Array` — reads `preferences.json` per mod
  - `installMod(mod)` — downloads `style`, `readme`, `preferences` from HTTPS theme store
  - `removeMod(modId, triggerUpdate)` — deletes folder + entry
  - `enableMod(modId)` / `disableMod(modId)` — toggles `.enabled` boolean
  - `checkForModsUpdates()` — fetches latest version from store, replaces outdated
  - `requestMod(modId)` → `object|null` — fetches from `https://zen-browser.github.io/theme-store/themes/{modId}/theme.json`
  - `isModInstalled(modId)` → `boolean`
  - `updateMods(mods)` — writes JSON, triggers mod update
  - `triggerModsUpdate()` — sets `zen.mods.updated-value-observer` pref toggles
  - `sanitizeModName(aName)` → `"theme-{sanitized}"`
  - `checkForModChanges()` — installs missing mod folders
- **Pref gates**:
  - `zen.themes.disable-all` — disables all mods
  - `zen.mods.auto-update` + `zen.mods.auto-update-days`
  - `zen.mods.milestone` — tracks build version
- **Security**: Only HTTPS mod asset URLs allowed. `no-cors` fetch to store API.
- **Dom injection**: `#writeToDom()` injects `--variable` CSS custom properties into each browser window's `<html>` for dropdown/string preferences. Creates `<div id="{sanitizedName}">` elements for dropdown attributes.

---

### 11. Theme System (`src/zen/common/zenThemeModifier.js`)

#### `ZenThemeModifier`
- **File**: `zenThemeModifier.js` — loaded via `<script>` tag in Chrome pages
- **Prefs monitored**:
  - `zen.theme.accent-color` → sets `--zen-primary-color`
  - `zen.theme.border-radius` → sets `--zen-border-radius` (platform defaults: macOS 10/14, Linux GTK, Windows 8)
  - `zen.theme.content-element-separation` → sets `--zen-element-separation` (max 12px)
- **Events**: Listens for split view, fullscreen, compact mode changes to recalculate separation
- **Fullscreen**: When `zen.view.borderless-fullscreen` is true and in native fullscreen (not DOM), separation = 0
- **Note**: Must be a Firefox builtin page with `Services` access.

---

### 12. Keyboard Shortcuts (`src/zen/kbs/`)

#### `ZenKeyboardShortcutsManager`
- **File**: `ZenKeyboardShortcuts.mjs`
- **Global**: `window.gZenKeyboardShortcutsManager`
- **Storage**: `zen-keyboard-shortcuts.json` in profile
- **Current version**: `LATEST_KBS_VERSION = 18`
- **Key classes**:
  - `nsKeyShortcutModifiers` — ctrl, alt, shift, meta, accel. Platform-aware (macOS uses accel→⌘, meta treated differently).
  - `KeyShortcut` — represents one keybinding: id, key, keycode, group, modifiers, action (command ID), l10nId, disabled, reserved, internal.
  - `nsZenKeyboardShortcutsLoader` — load/save/remove JSON. `zenGetDefaultShortcuts()` builds from main keyset + compact mode + workspace + split view + devtools shortcuts.
  - `nsZenKeyboardShortcutsVersioner` — migration chain (v0→v18), fixes conflicts, adds new defaults.
- **Public methods** (on `gZenKeyboardShortcutsManager`):
  - `getModifiableShortcuts()` → `KeyShortcut[]`
  - `setShortcut(action, shortcut, modifiers)` — updates binding
  - `resetAllShortcuts()` — removes file + pref
  - `checkForConflicts(shortcut, modifiers, id)` → `{ hasConflicts, conflictShortcut }`
  - `getShortcutFromCommand(command)` → `KeyShortcut|null`
  - `getShortcutDisplayFromCommand(command)` → `string|null` — human-readable
  - `triggerShortcutRebuild()` — re-applies all shortcuts to DOM keysets
- **Architecture**: Replaces Firefox's built-in `mainKeyset` with `zenKeyset`. Clears original `mainKeyset` children, inserts zen keyset after. Devtools shortcuts live in separate `zen-devtoolsKeyset`.
- **Groups**: `windowAndTabManagement`, `navigation`, `searchAndFind`, `pageOperations`, `historyAndBookmarks`, `mediaAndDisplay`, `devTools`, `zen-compact-mode`, `zen-workspace`, `zen-split-view`, `zen-other`

---

### 13. URL Bar (`src/zen/urlbar/`)

#### `ZenUrlbarProviderGlobalActions`
- **File**: `ZenUBActionsProvider.sys.mjs`
- **Extends**: `UrlbarProvider` (Firefox's built-in URL bar provider API)
- **Type**: `PROVIDER_TYPE.HEURISTIC`
- **Method**: `startQuery(queryContext, addCallback)` — fuzzy matches query against global actions
- **Fuzzy scoring**: VS Code-style (`#calculateFuzzyScore`): exact match=200, prefix match=100+len, consecutive char bonus, word-boundary bonus, distance penalty.
- **Sources**: `globalActions` template + workspace switching actions + extension actions
- **Result type**: `DYNAMIC` with `DYNAMIC_TYPE_NAME = "zen-actions"`
- **Max results**: 5 (non-prefixed), all (prefixed)
- **Learning**: Uses `ZenUrlbarResultsLearner` to prioritize/deprioritize results
- **Pref gate**: `zen.urlbar.suggestions.quick-actions` (default true)
- **Engagement**: On select, executes command ID (doCommand) or workspace switch or extension trigger

#### `globalActions` Template
- **File**: `ZenUBGlobalActions.sys.mjs`
- **29+ actions**: Toggle Compact Mode, Open Theme Picker, New Split View, New Folder, Copy URL, Settings, Private Window, New Window, Pin/Unpin Tab, Next/Previous Space, Close Tab, Reload, Next/Previous Tab, Screenshot, Toggle Tabs on Right, Add/Remove Essentials, Find in Page, Manage Extensions, Switch Light/Dark/Auto Appearance, Print
- **Each action**: `{ label, command (string or function), icon, isAvailable(window) }`
- **Dynamic availability**: checks disabled command state, empty tab state, pinned state, etc.

#### `ZenUrlbarResultsLearner`
- **File**: `ZenUBResultsLearner.sys.mjs`
- **Singleton**: `zenUrlbarResultsLearner`
- **Storage**: `zen.urlbar.suggestions-learner` pref (JSON string)
- **Methods**: `recordExecution(commandId, seenCommands)`, `shouldPrioritize(commandId)`, `getDeprioritizeIndex(commandId)`, `sortCommandsByPriority(commands)`
- **Scale**: PRIORITIZE_MAX=5, DEPRIORITIZE_MAX=-5
- **Logic**: executed command +1, seen-but-not-executed -1. Delete neutral (0).

#### `ZenSiteDataPanel`
- **File**: `ZenSiteDataPanel.sys.mjs`
- **Class**: `nsZenSiteDataPanel`
- **Replaces**: Firefox's identity/permissions panel with unified panel
- **Features**: Site permissions toggles, boost integration, addons overflow, copy URL button, security info, bookmark/share buttons, reader mode
- **Pref gate**: `zen.theme.hide-unified-extensions-button` (default true) — replaces extensions panel
- **Methods**: `checkIfTabIsBoosted()`, `#setSiteBoost()`, `#setSitePermissions()`, `#setSiteSecurityInfo()`, `#setSiteHeader()`

---

### 14. Media Controls (`src/zen/media/`)

#### `nsZenMediaController`
- **File**: `ZenMediaController.mjs`
- **Global**: `window.gZenMediaController`
- **Location**: Bottom of sidebar (controls bar)
- **Init pref**: `zen.mediacontrols.enabled` (default true)
- **Media API**: Uses `browsingContext.mediaController` (Firefox's MediaController API)
- **Key methods**:
  - `init()` — sets up DOM refs + event listeners
  - `activateMediaControls(mediaController, browser)` — listens to positionstatechange, playbackstatechange, metadatachange, supportedkeyschange, deactivated, pictureinpicturemodechange
  - `activateMediaDeviceControls(browser)` — for WebRTC sharing
  - `updateMediaSharing(data)` — microphone/camera indicators
  - `setupMediaController(mediaController, browser)` — sets current controller
  - `setupMediaControlUI(metadata, positionState)` — updates title/artist/progress
  - `switchController(force)` — auto-switches to most recently updated playing controller
  - `hideMediaControls()` / `showMediaControls()` — motion animation
  - `onMediaToggle()`, `onMediaPlayPrev()`, `onMediaPlayNext()`, `onMediaFocus()`, `onMediaMute()`, `onMediaPip()`, `onMediaSeekDrag()`, `onMediaSeekComplete()`
  - `onControllerClose()`, `onMicrophoneMuteToggle()`, `onCameraMuteToggle()`
- **Supported keys**: playpause, previoustrack, nexttrack
- **Position update**: 1-second interval timer
- **Duration cap**: Hides progress bar if duration >= 900,000ms (15 min)
- **WebRTC**: handles `webrtc:MuteMicrophone`, `webrtc:UnmuteMicrophone`, etc.
- **PIP**: checks `PictureInPicture.getEligiblePipVideoCount()`, toggles `can-pip` attribute

---

### 15. Window Sync (`src/zen/sessionstore/`)

#### `nsZenWindowSync`
- **File**: `ZenWindowSync.sys.mjs`
- **Concept**: All windows share the same tab sidebar. Only one window "owns" the real browser (docshell) for each tab at a time. Switching tabs/windows swaps docshells.
- **Key pref gates**:
  - `zen.window-sync.enabled` (default true)
  - `zen.window-sync.sync-only-pinned-tabs` (default true) — only pinned tabs are synced across windows
- **Public methods**:
  - `init()` / `uninit()` — observers setup
  - `getItemFromWindow(aWindow, aItemId)` → tab/group element
  - `addSyncHandler(aHandler)` / `removeSyncHandler(aHandler)` — extensibility
  - `setPinnedTabState(aTab)` — saves initial URL/title/image for pinned tabs
  - `propagateWorkspacesToAllWindows(aWorkspaces)` — syncs workspace list
  - `moveTabsToSyncedWorkspace(aWindow, aWorkspaceId)` — moves all tabs from unsynced window
- **Event handlers**: `on_TabOpen`, `on_TabClose`, `on_TabPinned`, `on_TabUnpinned`, `on_ZenTabIconChanged`, `on_ZenTabLabelChanged`, `on_TabMove`, `on_TabHide`, `on_TabShow`, `on_TabAddedToEssentials`, `on_TabRemovedFromEssentials`, `on_TabSelect`, `on_focus`, `on_SSWindowClosing`, `on_WindowCloseAndBrowserFlushed`, `on_TabGroupCreate`/`Update`/`Removed`/`Moved`, `on_ZenTabRemovedFromSplit`, `on_ZenSplitViewTabsSplit`
- **Core mechanism**: `#swapBrowserDocShellsAsync(aOurTab, aOtherTab)` — swaps the actual browser rendering between windows using `gBrowser.swapBrowsersAndCloseOther()`. Creates pseudo-screenshot images during swap for smooth transition.
- **Sync model**: Uses `_zenContentsVisible` flag on tabs. Only the "active" window's tab has `_zenContentsVisible = true`. On tab switch or window focus, swaps docshells.
- **Unsynced windows**: Identified by `zen-unsynced-window` attribute. Opened via `cmd_zenNewNavigatorUnsynced`.

#### `nsZenSessionManager`
- **File**: `ZenSessionManager.sys.mjs`
- **Singleton**: `ZenSessionStore`
- **File**: `zen-sessions.jsonlz4` in profile (LZ4 compressed JSON)
- **Backups**: `zen-sessions-backup/` folder, `zen-sessions-YYYY-MM-DD-HH.jsonlz4` format, max backups configurable (`zen.session-store.max-backups`, default 20), backup interval configurable (`zen.session-store.backup-hour-span`, default 3h)
- **Public methods**:
  - `init()`, `readFile()`, `onFileRead(initialState)`, `onCrashCheckpoints(initialState)`
  - `saveState(state, soon)` — collects sidebar data (tabs, folders, splitViewData, groups, spaces)
  - `maybeSaveClosedWindow(aWinData, isLastWindow)`
  - `restoreNewWindow(aWindow, SessionStoreInternal, fromClosedWindow)` — clones session data for new window
  - `onNewEmptySession(aWindow)` — restores only spaces
  - `getClonedSpaces()` → `Array`
  - `onRestoringClosedWindow(aWinData)` — filters unpinned tabs
- **Migration**: From Places DB (`zen_workspaces`, `zen_pins` tables) → JSON session file
- **Restoration logic**: Respects `browser.startup.page`, crash recovery, build ID changes

---

### 16. Drag-and-Drop (`src/zen/drag-and-drop/`)

#### `ZenDragAndDrop`
- **File**: `ZenDragAndDrop.js`
- **Class**: `ZenDragAndDrop extends TabDragAndDrop`
- **Key methods**:
  - `init()` — adds workspace icon drag-over handler, window drag-leave
  - `startTabDrag(event, tab, ...args)` — custom drag image (canvas + tab clones), essentials visibility
  - `_animateTabMove(event)` — vertical tab reorder with overlap-based drop detection
  - `handle_dragover(event)` — split-view drop, workspace switch on edge hover
  - `handle_drop(event)` — workspace switching on drop, split creation
  - `handle_drop_transition(dropElement, draggedTab, movingTabs, dropBefore)` — animated tab movement (translateY spring animations)
  - `handle_dragend(event)` — cleanup
- **Split-view drop**: Drag tab to left/right half of another tab → creates split view after delay (`zen.splitView.drag-over-split-delayMC`, default 300ms). Threshold: `zen.splitView.drag-over-split-threshold` (default 25%).
- **Workspace switch on drag**: Hovering near left/right edge of sidebar → workspace change after `zen.tabs.dnd-switch-space-delay` (default 1000ms). Also supports direct workspace icon drag-over.
- **Essentials grid**: Special handling for `#zen-essentials` container, vertical pinned grid drag-over.
- **Cross-window**: When dragging outside window (`zen.tabs.dnd-outside-window-margin`), creates `_browserDragImageWrapper` with canvas + tab count dot.
- **Native service**: Uses `@mozilla.org/zen/drag-and-drop;1` (`nsIZenDragAndDrop`) for native drag image opacity.
- **Pref gates**:
  - `zen.splitView.enable-drag-over-split` (default true)
  - `zen.tabs.folder-dragover-threshold-percent`
  - `browser.tabs.dragDrop.moveOverThresholdPercent`

---

### 17. Downloads (`src/zen/downloads/`)

#### `nsZenDownloadAnimation`
- **File**: `ZenDownloadAnimation.mjs`
- **Singleton class**: `nsZenDownloadAnimation extends nsZenDOMOperatedFeature`
- **Trigger**: `Downloads.getList(Downloads.ALL).addView()` → `onDownloadAdded`
- **Pref gate**: `zen.downloads.download-animation` (default true)
- **Animation**: Arc from `gZenUIManager._lastClickPosition` to download button. Uses `gZenUIManager.motion.animate()` for motion design.
- **Custom element**: `<zen-download-animation>` (defined in same file as `nsZenDownloadAnimationElement extends HTMLElement`)
- **Arc math**: 60-step cubic Bezier arc with `easeInOutQuad`. Max arc height = `distance * 0.8` capped at 1200px. Scale: 0.5→1.8→0.45.
- **Box animation**: If download button not visible, shows box animation at bottom-left/right of wrapper (position depends on `zen.tabs.vertical.right-side`).
- **No dependencies** beyond motion system and download list

---

### 18. Welcome (`src/zen/welcome/`)

#### `ZenWelcome`
- **File**: `ZenWelcome.mjs`
- **Auto-starts**: `startZenWelcome()` called at script load
- **Stages** (5 pages):
  1. **Import** — "Would you like to import?" with visual tabs. Option to set as default browser.
  2. **Default Search Engine** — Select from installed search engines
  3. **Initial Essentials** — Pick essential tabs (Obsidian, Discord, Trello, Slack, GitHub, Tuta, Notion, Calendar, Figma). Creates pinned tabs from selection.
  4. **Workspace Colors** — Opens theme picker panel inline (non-native popover). `gZenThemePicker.panel`.
  5. **Start Browsing** — Final page, closes welcome, restores normal UI
- **Animation system**: `gZenUIManager.motion` (spring, stagger, fade). `animate()` wrapper.
- **Class**: `nsZenWelcomePages` — manages page transitions
- **Class**: `ZenSearchEngineStore` — wraps search engine selection
- **Window**: Centers on screen (875×560), removes browser chrome elements except `zen-browser-background` and `zen-toast-container`
- **Post-welcome**: `gZenWorkspaces.reorganizeTabsAfterWelcome()`, pins essentials, creates "zen basics" folder, shows toast "zen-welcome-finished"

---

### 19. Emoji Picker (`src/zen/common/emojis/`)

#### `nsZenEmojiPicker`
- **File**: `ZenEmojiPicker.mjs`
- **Global**: `window.gZenEmojiPicker`
- **Extends**: `nsZenDOMOperatedFeature`
- **Panel ID**: `PanelUI-zen-emojis-picker`
- **Data**: `ZenEmojisData.min.mjs` (minified emoji list with tags and order)
- **82 SVG icons**: from `chrome://browser/skin/zen-icons/selectable/` (airplane, heart, star, rocket, etc.)
- **Public method**: `open(anchor, { onlySvgIcons, emojiAsSVG, allowNone, closeOnSelect, onSelect })` → `Promise<string|null>`
- **Features**: Search (by tag), two pages (emojis ↔ SVG icons), SVG-as-emoji conversion (base64 data URI)
- **Behavior**: Promise-based API. Resolves with selected emoji/SVG URL on close, rejects if closed without selection (unless `closeOnSelect=false` + `onSelect` callback).

---

## Architecture

### Data Flow Summary

```
┌─────────────────────────────────────────────────────────────┐
│ Boosts: per-domain CSS injection via nsIStyleSheetService   │
│  Profile files: zen-boosts.jsonlz4 + zen-boosts/{id}.css    │
│  Notifications: zen-boosts-update, zen-boosts-active-change  │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│ Mods: zen-themes.json + per-mod folder (chrome.css,         │
│  preferences.json, readme.md) → compiled zen-themes.css     │
│  + DOM variables injected into each browser window          │
│  Fetches from zen-browser.github.io/theme-store             │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│ Theme Modifier: Injected into every Chrome page via          │
│  <script src="zenThemeModifier.js">. Sets CSS vars on        │
│  document.documentElement from prefs.                        │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│ Keyboard Shortcuts: Replaces Firefox main keyset.            │
│  Profile file: zen-keyboard-shortcuts.json                   │
│  Version: 18 (migration chain preserves user changes)        │
│  Groups: window/tab, navigation, search, page ops, devtools  │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│ URL Bar: Heuristic provider (ZenUrlbarProviderGlobalActions) │
│  Fuzzy matches against global actions + workspaces +         │
│  extensions. Learned prioritization via pref storage.        │
│  Site Data Panel replaces identity panel with unified UI.    │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│ Media Controls: Sidebar bar using Firefox MediaController    │
│  API. Auto-switches between playing tabs, handles WebRTC.    │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│ Window Sync: All browser windows share same tabbar state.    │
│  Only one window "owns" each tab's docshell. Tab switch      │
│  swaps docshells between windows via swapBrowsersAndCloseOther│
│  Pinned tabs synced across windows, unpinned tabs optional.  │
│  Session: zen-sessions.jsonlz4 (sidebar persistence)         │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│ Drag-and-Drop: Extends Firefox TabDragAndDrop.               │
│  Custom drag images, positional animations, split-view drop, │
│  workspace switch on edge hover, essentials grid support.    │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│ Downloads: Listens to Downloads API for new downloads.       │
│  Arc animation from last click position to download button.  │
│  Box animation fallback when button hidden.                  │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│ Welcome: Multi-stage onboarding wizard. Sets default browser,│
│  search engine, essential tabs, workspace colors.            │
│  Uses motion animation system, theme picker inline.          │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│ Emoji Picker: Promise-based panel. Two pages: emojis + SVG  │
│  icons. Searchable by tag. Convertible to SVG data URIs.     │
└─────────────────────────────────────────────────────────────┘
```

### Restrictions & Pref Gates Summary

| Feature | Pref | Default | Effect |
|---------|------|---------|--------|
| Boosts | (none for manager itself) | — | Only http/https URIs |
| Mods | `zen.themes.disable-all` | false | Disables all mods |
| Mods | `zen.mods.auto-update` | true | Auto-update enabled |
| Mods | `zen.mods.auto-update-days` | ? | Days between update checks |
| Theme | `zen.theme.accent-color` | "" | Accent (empty = system) |
| Theme | `zen.theme.border-radius` | -1 | -1 = platform default |
| Theme | `zen.theme.content-element-separation` | ? | Px separation |
| Theme | `zen.view.borderless-fullscreen` | true | Zero separation in fullscreen |
| KBS | `zen.keyboard.shortcuts.version` | 0→18 | Migration version |
| KBS | `zen.keyboard.shortcuts.disable-mainkeyset-clear` | false | Skip clearing main keyset |
| URL Bar | `zen.urlbar.suggestions.quick-actions` | true | Enable quick actions |
| URL Bar | `zen.urlbar.suggestions-learner` | "{}" | Learning database |
| URL Bar | `zen.site-data-panel.show-callout` | false | Feature callout |
| URL Bar | `zen.theme.hide-unified-extensions-button` | true | Replace extensions panel |
| Media | `zen.mediacontrols.enabled` | true | Show media controls |
| Window Sync | `zen.window-sync.enabled` | true | Cross-window sync |
| Window Sync | `zen.window-sync.sync-only-pinned-tabs` | true | Only sync pinned |
| Window Sync | `zen.session-store.backup-file` | true | Backup sessions |
| Window Sync | `zen.session-store.max-backups` | 20 | Max backup files |
| Session | `browser.startup.page` | 1 | 3 = resume session |
| Downloads | `zen.downloads.download-animation` | true | Arc animation |
| Drag | `zen.splitView.enable-drag-over-split` | true | Split drag |
| Drag | `zen.splitView.drag-over-split-threshold` | 25 | Split zone % |
| Drag | `zen.splitView.drag-over-split-delayMC` | 300 | Split hover delay ms |
| Drag | `zen.tabs.dnd-switch-space-delay` | 1000 | Workspace switch delay |
| Drag | `zen.tabs.dnd-outside-window-margin` | 5 | Cross-window margin |
| Emoji | (none) | — | No pref gate |

### Platform-Specific Behavior

- **Keyboard shortcuts**: macOS uses ⌘ (accel/meta), others use Ctrl. `AppConstants.platform === "macosx"` changes modifier display + equality logic.
- **Theme border-radius**: macOS 10px/14px (Tahoe), Linux `env(-moz-gtk-csd-titlebar-radius, 8px)`, Windows 8px.
- **Drag images**: Non-macOS platforms force `colorScheme: "light"` + black text for visibility (workaround for lack of native drag image opacity).
- **Downloads arc animation**: Uses `zen.tabs.vertical.right-side` pref (position 0=auto, 1=left, 2=right) for target positioning.

### @IS_TWILIGHT@ Gates
- No `@IS_TWILIGHT@` gates found in these files. None of these features are restricted to Twilight channel.

---

## Start Here

For zenctl integration, start with:
1. **`ZenBoostsManager.sys.mjs`** — Boosts CRUD is the richest API surface
2. **`ZenMods.mjs`** — Mod management (install/enable/disable)
3. **`zenThemeModifier.js`** — Theme prefs (already supported via zenPrefs experiment)
4. **`ZenKeyboardShortcuts.mjs`** — Shortcut management (already has `setShortcut`, `getModifiableShortcuts`, `resetAllShortcuts`)
5. **`ZenSessionManager.sys.mjs`** — Session backup/restore (profile file method candidates)

Existing zenctl coverage for these features is documented in `docs/zenctl-feature-audit.md`.
