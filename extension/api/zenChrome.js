"use strict";

// Privileged operations against a live Zen chrome window. Runs in the parent
// process; reaches into the most recently focused navigator:browser window
// and calls Zen's own gZen* managers.

/* global ExtensionAPI, Services, ChromeUtils */

// ExtensionError is a DOMException subclass whitelisted by the
// WebExtension experiment-API bridge: thrown instances keep their message
// when crossing the parent-process boundary. Plain `new Error("...")`
// throws get sanitized to "An unexpected error occurred @undefined:0".
const { ExtensionError } = ChromeUtils.importESModule(
  "resource://gre/modules/ExtensionUtils.sys.mjs"
);

this.zenChrome = class extends ExtensionAPI {
  getAPI(context) {
    function topWindow() {
      const win = Services.wm.getMostRecentWindow("navigator:browser");
      if (!win) {
        throw new Error("no navigator:browser window open");
      }
      return win;
    }

    function compactMgr(win) {
      const mgr = win.gZenCompactModeManager;
      if (!mgr) {
        throw new Error("gZenCompactModeManager unavailable on this window");
      }
      return mgr;
    }

    // Mutate one field of a cached workspace and persist it. Zen has no
    // dedicated rename/set-icon API — saveWorkspace() takes the whole record.
    function saveWorkspaceField(uuid, field, value) {
      const ws = topWindow().gZenWorkspaces;
      if (!ws || typeof ws.saveWorkspace !== "function") {
        throw new Error("gZenWorkspaces.saveWorkspace unavailable");
      }
      const cache = Array.isArray(ws._workspaceCache) ? ws._workspaceCache : [];
      const workspace = cache.find((w) => w.uuid === uuid);
      if (!workspace) {
        throw new Error("workspace not found: " + uuid);
      }
      workspace[field] = value;
      ws.saveWorkspace(workspace);
      return { uuid, [field]: value };
    }

    function pinnedMgr(win) {
      const mgr = win.gZenPinnedTabManager;
      if (!mgr) {
        throw new ExtensionError(
          "gZenPinnedTabManager unavailable on this window"
        );
      }
      return mgr;
    }

    // Parse the host's JSON array of tab URLs, rejecting anything that is not
    // a non-empty array.
    function parseUrls(tabUrlsJson) {
      let urls;
      try {
        urls = tabUrlsJson ? JSON.parse(tabUrlsJson) : [];
      } catch (e) {
        throw new ExtensionError(
          `invalid tab urls JSON: ${String(e?.message ?? e)}`
        );
      }
      if (!Array.isArray(urls) || urls.length === 0) {
        throw new ExtensionError("at least one tab url required");
      }
      return urls;
    }

    function importZenModule(uri, symbol) {
      try {
        const mod = ChromeUtils.importESModule(uri, { global: "current" });
        const value = mod[symbol];
        if (!value) throw new ExtensionError(`${symbol} missing from ${uri}`);
        return value;
      } catch (e) {
        if (e instanceof ExtensionError) throw e;
        throw new ExtensionError(`${symbol} unavailable: ${String(e?.message ?? e)}`);
      }
    }

    // Match XUL <tab> elements in one window by linkedBrowser current URL.
    function matchTabsByUrl(win, urls) {
      const matched = [];
      if (win.gBrowser?.tabContainer) {
        for (const t of win.gBrowser.tabContainer.querySelectorAll("tab")) {
          const u = t.linkedBrowser?.currentURI?.spec;
          if (u && urls.includes(u)) matched.push(t);
        }
      }
      return matched;
    }

    return {
      zenChrome: {
        async compactSet(value) {
          const win = topWindow();
          const mgr = compactMgr(win);
          mgr.preference = !!value;
          return { enabled: !!mgr.preference };
        },

        async compactToggle() {
          const win = topWindow();
          const mgr = compactMgr(win);
          const next = !mgr.preference;
          mgr.preference = next;
          return { enabled: !!mgr.preference };
        },

        async workspaceSwitch(uuid) {
          const win = topWindow();
          const ws = win.gZenWorkspaces;
          if (!ws || typeof ws.changeWorkspaceWithID !== "function") {
            throw new Error("gZenWorkspaces.changeWorkspaceWithID unavailable");
          }
          await ws.changeWorkspaceWithID(uuid);
          return { active: uuid };
        },

        async glanceClose() {
          const win = topWindow();
          const mgr = win.gZenGlanceManager;
          if (!mgr || typeof mgr.closeGlance !== "function") {
            throw new Error("gZenGlanceManager.closeGlance unavailable");
          }
          mgr.closeGlance();
          return { closed: true };
        },

        async glanceExpand() {
          const win = topWindow();
          const mgr = win.gZenGlanceManager;
          if (!mgr || typeof mgr.fullyOpenGlance !== "function") {
            throw new Error("gZenGlanceManager.fullyOpenGlance unavailable");
          }
          mgr.fullyOpenGlance();
          return { expanded: true };
        },

        async splitViewCreate(tabUrlsJson, gridType = "grid") {
          try {
            const tabUrls = tabUrlsJson ? JSON.parse(tabUrlsJson) : [];
            // Search all browser windows for the target tabs. `gBrowser.tabs`
            // getter can crash when group state is inconsistent, so walk DOM
            // container directly.
            const findTabByUrl = (url) => {
              const e = Services.wm.getEnumerator("navigator:browser");
              while (e.hasMoreElements()) {
                const w = e.getNext();
                if (!w.gBrowser?.tabContainer) continue;
                for (const t of w.gBrowser.tabContainer.querySelectorAll("tab")) {
                  if (t.linkedBrowser?.currentURI?.spec === url) {
                    return { tab: t, win: w };
                  }
                }
              }
              return null;
            };

            if (tabUrls.length >= 2) {
              const matches = tabUrls.map(findTabByUrl).filter(Boolean);
              if (matches.length < 2) {
                return { created: false, error: `only ${matches.length} matched`, want: tabUrls };
              }
              const win = matches[0].win;
              // All tabs must be in same window for split. Move strays if needed.
              for (const m of matches.slice(1)) {
                if (m.win !== win) {
                  return { created: false, error: "tabs span multiple windows" };
                }
              }
              const splitter = win.gZenViewSplitter;
              if (!splitter) {
                return { created: false, error: "gZenViewSplitter unavailable" };
              }
              const tabs = matches.map(m => m.tab);
              // Clear stale split state: tab.splitView true but not in _data.
              // Without this, splitTabs() crashes on `group.tabs.length` when
              // groupIndex is -1.
              for (const t of tabs) {
                if (t.splitView) {
                  const inGroup = splitter._data?.some(g => g.tabs?.includes(t));
                  if (!inGroup) splitter.resetTabState(t, false);
                }
              }
              splitter.splitTabs(tabs, gridType);
            } else if (tabUrls.length === 1) {
              const m = findTabByUrl(tabUrls[0]);
              if (!m) return { created: false, error: "tab not found" };
              const splitter = m.win.gZenViewSplitter;
              if (!splitter) return { created: false, error: "gZenViewSplitter unavailable" };
              m.win.gBrowser.selectedTab = m.tab;
              splitter.contextSplitTabs();
            } else {
              const win = topWindow();
              const splitter = win.gZenViewSplitter;
              if (!splitter) return { created: false, error: "gZenViewSplitter unavailable" };
              splitter.contextSplitTabs();
            }
            return { created: true };
          } catch (e) {
            const stack = e?.stack ? String(e.stack).split("\n").slice(0, 6).join(" || ") : "";
            return { created: false, error: String(e), stack };
          }
        },

        async workspacesList() {
          const win = topWindow();
          const ws = win.gZenWorkspaces;
          if (!ws) throw new Error("gZenWorkspaces unavailable");
          const cache = Array.isArray(ws._workspaceCache) ? ws._workspaceCache : [];
          const workspaces = cache.map((w) => ({
            uuid: w.uuid,
            name: w.name ?? null,
            icon: w.icon ?? null,
            container_id: w.containerTabId ?? null,
            position: w.position ?? null,
            theme: w.theme ?? null,
          }));
          const activeWs =
            (typeof ws.getActiveWorkspace === "function" && ws.getActiveWorkspace()) ||
            (typeof ws.getActiveWorkspaceFromCache === "function" && ws.getActiveWorkspaceFromCache()) ||
            null;
          const active = activeWs?.uuid || null;
          return { active, workspaces };
        },

        async workspaceCreate(name, icon) {
          const ws = topWindow().gZenWorkspaces;
          if (!ws || typeof ws.createAndSaveWorkspace !== "function") {
            throw new Error("gZenWorkspaces.createAndSaveWorkspace unavailable");
          }
          const created = await ws.createAndSaveWorkspace(
            name || "Space",
            icon || undefined,
            false,
            0
          );
          return {
            created: !!created,
            uuid: created?.uuid ?? null,
            name: created?.name ?? name,
          };
        },

        async workspaceRemove(uuid) {
          const ws = topWindow().gZenWorkspaces;
          if (!ws || typeof ws.removeWorkspace !== "function") {
            throw new Error("gZenWorkspaces.removeWorkspace unavailable");
          }
          await ws.removeWorkspace(uuid);
          return { removed: uuid };
        },

        async workspaceRename(uuid, name) {
          return saveWorkspaceField(uuid, "name", name);
        },

        async workspaceSetIcon(uuid, icon) {
          return saveWorkspaceField(uuid, "icon", icon);
        },

        async workspaceSetContainer(uuid, cookieStoreId) {
          // Parse firefox-container-N → N, or raw number, or "0"/"firefox-default" → 0
          const num = parseInt((cookieStoreId || "0").split("-").pop(), 10);
          if (isNaN(num) || num < 0) {
            throw new Error("invalid container: " + cookieStoreId + " (use firefox-container-N, N, or firefox-default)");
          }
          return saveWorkspaceField(uuid, "containerTabId", num);
        },

        async workspaceReorder(uuid, index) {
          const ws = topWindow().gZenWorkspaces;
          if (!ws || typeof ws.reorderWorkspace !== "function") {
            throw new Error("gZenWorkspaces.reorderWorkspace unavailable");
          }
          await ws.reorderWorkspace(uuid, index);
          return { uuid, index };
        },

        async workspaceUnload(workspaceId, others = false) {
          const win = topWindow();
          const ws = win.gZenWorkspaces;
          if (!ws || !win.gBrowser?.explicitUnloadTabs) {
            throw new ExtensionError("gZenWorkspaces/gBrowser.explicitUnloadTabs unavailable");
          }
          const activeWs =
            (typeof ws.getActiveWorkspace === "function" && ws.getActiveWorkspace()) ||
            (typeof ws.getActiveWorkspaceFromCache === "function" && ws.getActiveWorkspaceFromCache()) ||
            null;
          const targetId = workspaceId || activeWs?.uuid;
          if (!targetId) throw new ExtensionError("workspace id required");
          const cache = Array.isArray(ws._workspaceCache) ? ws._workspaceCache : [];
          if (!cache.some((w) => w.uuid === targetId)) {
            throw new ExtensionError(`workspace not found: ${targetId}`);
          }
          const tabsToUnload = (Array.isArray(ws.allStoredTabs) ? ws.allStoredTabs : []).filter(
            (tab) =>
              (others
                ? tab.getAttribute("zen-workspace-id") !== targetId
                : tab.getAttribute("zen-workspace-id") === targetId) &&
              !tab.hasAttribute("zen-empty-tab") &&
              !tab.hasAttribute("zen-essential") &&
              !tab.hasAttribute("pending")
          );
          if (tabsToUnload.length > 0) {
            await win.gBrowser.explicitUnloadTabs(tabsToUnload);
          }
          return { workspace_id: targetId, others: !!others, unloaded: tabsToUnload.length };
        },

        async workspaceMoveTab(workspaceId, tabUrlsJson) {
          if (!workspaceId) throw new ExtensionError("workspace id required");
          const win = topWindow();
          const ws = win.gZenWorkspaces;
          if (!ws || typeof ws.moveTabsToWorkspace !== "function") {
            throw new ExtensionError("gZenWorkspaces.moveTabsToWorkspace unavailable");
          }
          // moveTabsToWorkspace silently no-ops on an unknown workspace id
          // (workspaceElement() returns undefined → no container). Fail fast
          // instead so the caller gets a clear error, not a quiet skip.
          const cache = Array.isArray(ws._workspaceCache) ? ws._workspaceCache : [];
          if (!cache.some((w) => w.uuid === workspaceId)) {
            throw new ExtensionError(`workspace not found: ${workspaceId}`);
          }
          let urls;
          try {
            urls = tabUrlsJson ? JSON.parse(tabUrlsJson) : [];
          } catch (e) {
            throw new ExtensionError(`invalid tab urls JSON: ${String(e?.message ?? e)}`);
          }
          if (!Array.isArray(urls) || urls.length === 0) {
            throw new ExtensionError("at least one tab url required");
          }
          // Workspace tab containers are window-scoped (workspaceElement(id)
          // resolves within one window). Only match tabs in this window —
          // moveTabsToWorkspace's container.insertBefore() would otherwise
          // adopt a stray tab across windows.
          const matched = [];
          if (win.gBrowser?.tabContainer) {
            for (const t of win.gBrowser.tabContainer.querySelectorAll("tab")) {
              const u = t.linkedBrowser?.currentURI?.spec;
              if (u && urls.includes(u)) matched.push(t);
            }
          }
          if (matched.length === 0) {
            throw new ExtensionError(
              `no matching tabs in the active window for: ${urls.join(", ")}`
            );
          }
          // moveTabsToWorkspace silently skips zen-essential tabs and tabs
          // already in the target, then returns a bare `true`. Snapshot the
          // zen-workspace-id attribute around the call to report per tab.
          const before = matched.map((t) => ({
            url: t.linkedBrowser?.currentURI?.spec ?? null,
            workspace: t.getAttribute("zen-workspace-id"),
            essential: t.hasAttribute("zen-essential"),
          }));
          ws.moveTabsToWorkspace(matched, workspaceId);
          let moved = 0;
          let skipped = 0;
          const tabs = matched.map((t, i) => {
            const after = t.getAttribute("zen-workspace-id");
            const ok = after === workspaceId && before[i].workspace !== workspaceId;
            if (ok) moved++;
            else skipped++;
            return {
              url: before[i].url,
              moved: ok,
              reason: ok
                ? null
                : before[i].essential
                  ? "essential tab — workspace-bound"
                  : before[i].workspace === workspaceId
                    ? "already in target workspace"
                    : "not moved",
            };
          });
          return { workspace_id: workspaceId, moved, skipped, tabs };
        },

        async windowSyncForce() {
          const win = topWindow();
          const workspaces = win.gZenWorkspaces;
          if (!workspaces || typeof workspaces.propagateWorkspacesToAllWindows !== "function") {
            throw new ExtensionError("gZenWorkspaces.propagateWorkspacesToAllWindows unavailable");
          }
          workspaces.propagateWorkspacesToAllWindows();
          return { synced: true };
        },

        async compactHide(what) {
          const mgr = compactMgr(topWindow());
          const fn = { sidebar: "hideSidebar", toolbar: "hideToolbar", both: "hideBoth" }[what];
          if (!fn || typeof mgr[fn] !== "function") {
            throw new Error("compact hide target must be sidebar|toolbar|both");
          }
          mgr[fn]();
          return { hidden: what };
        },

        async glanceList() {
          const glances = [];
          const e = Services.wm.getEnumerator("navigator:browser");
          while (e.hasMoreElements()) {
            const win = e.getNext();
            if (!win.gBrowser?.tabContainer) continue;
            for (const tab of win.gBrowser.tabContainer.querySelectorAll("tab[zen-glance-tab][glance-id]")) {
              glances.push({
                id: tab.getAttribute("glance-id"),
                url: tab.linkedBrowser?.currentURI?.spec ?? null,
                label: tab.getAttribute("label") || tab.label || null,
                selected: !!tab.selected,
              });
            }
          }
          return { count: glances.length, glances };
        },

        async glanceCloseAll() {
          let closed = 0;
          const e = Services.wm.getEnumerator("navigator:browser");
          while (e.hasMoreElements()) {
            const win = e.getNext();
            if (!win.gBrowser?.tabContainer) continue;
            const tabs = Array.from(win.gBrowser.tabContainer.querySelectorAll("tab[zen-glance-tab][glance-id]"));
            for (const tab of tabs) {
              win.gBrowser.removeTab(tab, { animate: false });
              closed++;
            }
          }
          return { closed };
        },

        async glanceOpen(url) {
          const win = topWindow();
          const mgr = win.gZenGlanceManager;
          if (!mgr) throw new Error("gZenGlanceManager null");
          if (typeof mgr.openGlance !== "function") {
            throw new Error("openGlance not found on gZenGlanceManager");
          }
          // ZenGlanceManager.openGlance(data) needs:
          //   - triggeringPrincipal: gBrowser.addTab() rejects loads without
          //     one (see ZenGlanceManager.mjs line 81–87, the context-menu
          //     invocation always supplies the system principal).
          //   - clientX/clientY/width/height: drives the open animation
          //     origin. Falls back to lastLinkClickData, which is zero-filled
          //     until the user actually clicks a link — zero width/height
          //     breaks the animation. Use the tab content area as origin.
          const principal = Services.scriptSecurityManager.getSystemPrincipal();
          let pos = { clientX: 0, clientY: 0, width: 100, height: 100 };
          try {
            const rect = win.windowUtils.getBoundsWithoutFlushing(
              win.gBrowser.tabpanels
            );
            if (rect && rect.width && rect.height) {
              const w = 100;
              const h = 100;
              pos = {
                clientX: rect.width / 2 - w / 2,
                clientY: rect.height / 2 - h / 2,
                width: w,
                height: h,
              };
            }
          } catch (_) {
            /* fall through with defaults */
          }
          try {
            const result = mgr.openGlance({
              url,
              triggeringPrincipal: principal,
              ...pos,
            });
            if (result && typeof result.then === "function") {
              await result;
            }
            return { opened: true, url };
          } catch (e) {
            throw new Error(`glance open failed: ${String(e?.message ?? e)} [${e?.fileName}:${e?.lineNumber}]`);
          }
        },

        async urlbarSearch(query, submit) {
          if (query === undefined || query === null) {
            throw new ExtensionError("query required");
          }
          const win = topWindow();
          const urlbar = win.gURLBar;
          if (!urlbar) {
            throw new ExtensionError("gURLBar unavailable on this window");
          }
          if (typeof urlbar.search !== "function") {
            throw new ExtensionError("gURLBar.search unavailable (Firefox API change?)");
          }
          if (!submit) {
            // Open the address bar with the query and show the results
            // panel; the user finishes by picking a result or pressing Enter.
            urlbar.search(query);
            if (typeof urlbar.focus === "function") urlbar.focus();
            return { opened: true, submitted: false, query };
          }
          // --submit: execute the default action for the raw string (search
          // or navigate) without leaving the panel open for interaction.
          // handleNavigation is the modern UrlbarInput entry point;
          // handleCommand is the older name kept as a fallback.
          if (typeof urlbar.focus === "function") urlbar.focus();
          urlbar.value = query;
          if (typeof urlbar.setPageProxyState === "function") {
            urlbar.setPageProxyState("invalid", false);
          }
          let via;
          try {
            if (typeof urlbar.handleNavigation === "function") {
              via = "handleNavigation";
              urlbar.handleNavigation({});
            } else if (typeof urlbar.handleCommand === "function") {
              via = "handleCommand";
              urlbar.handleCommand();
            } else {
              throw new ExtensionError(
                "gURLBar has no handleNavigation/handleCommand"
              );
            }
          } catch (e) {
            if (e instanceof ExtensionError) throw e;
            throw new ExtensionError(
              `urlbar submit via ${via} failed: ${String(e?.message ?? e)}`
            );
          }
          return { opened: false, submitted: true, query, via };
        },

        async urlbarClose() {
          const win = topWindow();
          const urlbar = win.gURLBar;
          if (!urlbar) {
            throw new ExtensionError("gURLBar unavailable on this window");
          }
          // handleRevert() is the Esc-key behavior: restores the value to
          // the current page's URL and closes the open results panel.
          if (typeof urlbar.handleRevert === "function") {
            urlbar.handleRevert();
          }
          if (urlbar.view && typeof urlbar.view.close === "function") {
            urlbar.view.close();
          }
          if (typeof urlbar.blur === "function") {
            urlbar.blur();
          }
          return { closed: true };
        },

        async urlbarActionsList() {
          const actions = importZenModule("resource:///modules/ZenUBGlobalActions.sys.mjs", "globalActions");
          const win = topWindow();
          const list = actions.map((a, index) => ({
            id: a.id ?? a.l10nId ?? String(index),
            label: a.label ?? a.l10nId ?? a.id ?? String(index),
            l10n_id: a.l10nId ?? null,
            icon: a.icon ?? null,
            disabled: typeof a.isAvailable === "function" ? !a.isAvailable(win) : false,
          }));
          return { count: list.length, actions: list };
        },

        async urlbarActionsRun(action) {
          if (!action) throw new ExtensionError("action required");
          const actions = importZenModule("resource:///modules/ZenUBGlobalActions.sys.mjs", "globalActions");
          const win = topWindow();
          const needle = String(action).toLowerCase();
          const entry = actions.find((a, index) => {
            const id = String(a.id ?? a.l10nId ?? index).toLowerCase();
            const label = String(a.label ?? a.l10nId ?? a.id ?? index).toLowerCase();
            return id === needle || label === needle || id.includes(needle) || label.includes(needle);
          });
          if (!entry) throw new ExtensionError(`URL-bar action not found: ${action}`);
          if (typeof entry.isAvailable === "function" && !entry.isAvailable(win)) {
            throw new ExtensionError(`URL-bar action disabled: ${action}`);
          }
          if (typeof entry.command === "function") {
            await entry.command(win);
          } else if (typeof entry.command === "string") {
            const node = win.document.getElementById(entry.command);
            if (!node || typeof node.doCommand !== "function") {
              throw new ExtensionError(`URL-bar command unavailable: ${entry.command}`);
            }
            node.doCommand();
          } else {
            throw new ExtensionError("URL-bar action has no callable command");
          }
          return { ran: true, id: entry.commandId ?? entry.l10nId ?? null, label: entry.label ?? entry.l10nId ?? entry.commandId ?? null };
        },

        async essentialsList() {
          // Essential tabs are window-scoped; walk every browser window.
          const essentials = [];
          const e = Services.wm.getEnumerator("navigator:browser");
          while (e.hasMoreElements()) {
            const w = e.getNext();
            const container = w.gBrowser?.tabContainer;
            if (!container) continue;
            for (const t of container.querySelectorAll("tab")) {
              if (!t.hasAttribute("zen-essential")) continue;
              essentials.push({
                url: t.linkedBrowser?.currentURI?.spec ?? null,
                title: t.label ?? null,
                container_id: t.getAttribute("usercontextid") ?? null,
                pinned: !!t.pinned,
              });
            }
          }
          return { count: essentials.length, essentials };
        },

        async essentialsAdd(tabUrlsJson) {
          const win = topWindow();
          const mgr = pinnedMgr(win);
          if (!mgr.enabled) {
            throw new ExtensionError(
              "essentials disabled (private window or workspaces off)"
            );
          }
          const urls = parseUrls(tabUrlsJson);
          const matched = matchTabsByUrl(win, urls);
          if (matched.length === 0) {
            throw new ExtensionError(
              `no matching tabs in the active window for: ${urls.join(", ")}`
            );
          }
          // addToEssentials returns only a bare movedAll bool. Snapshot per
          // tab around the call to report each one.
          const before = matched.map((t) => ({
            url: t.linkedBrowser?.currentURI?.spec ?? null,
            wasEssential: t.hasAttribute("zen-essential"),
            canAdd: mgr.canEssentialBeAdded(t),
          }));
          mgr.addToEssentials(matched);
          let added = 0;
          let skipped = 0;
          const tabs = matched.map((t, i) => {
            const ok = t.hasAttribute("zen-essential") && !before[i].wasEssential;
            if (ok) added++;
            else skipped++;
            return {
              url: before[i].url,
              added: ok,
              reason: ok
                ? null
                : before[i].wasEssential
                  ? "already essential"
                  : !before[i].canAdd
                    ? "cannot add — essential limit reached or container mismatch"
                    : "not added",
            };
          });
          return { added, skipped, tabs };
        },

        async essentialsRemove(tabUrlsJson, unpin = true) {
          const win = topWindow();
          const mgr = pinnedMgr(win);
          const urls = parseUrls(tabUrlsJson);
          const matched = matchTabsByUrl(win, urls);
          if (matched.length === 0) {
            throw new ExtensionError(
              `no matching tabs in the active window for: ${urls.join(", ")}`
            );
          }
          const doUnpin = unpin !== false;
          let removed = 0;
          let skipped = 0;
          const tabs = matched.map((t) => {
            const url = t.linkedBrowser?.currentURI?.spec ?? null;
            if (!t.hasAttribute("zen-essential")) {
              skipped++;
              return { url, removed: false, reason: "not an essential tab" };
            }
            mgr.removeEssentials(t, doUnpin);
            const ok = !t.hasAttribute("zen-essential");
            if (ok) removed++;
            else skipped++;
            return { url, removed: ok, reason: ok ? null : "not removed" };
          });
          return { removed, skipped, unpinned: doUnpin, tabs };
        },

        async essentialsReset(tabUrlsJson) {
          const win = topWindow();
          const mgr = pinnedMgr(win);
          const urls = parseUrls(tabUrlsJson);
          const matched = matchTabsByUrl(win, urls);
          if (matched.length === 0) {
            throw new ExtensionError(
              `no matching tabs in the active window for: ${urls.join(", ")}`
            );
          }
          let reset = 0;
          let skipped = 0;
          const tabs = matched.map((t) => {
            const url = t.linkedBrowser?.currentURI?.spec ?? null;
            if (!t.pinned) {
              skipped++;
              return { url, reset: false, reason: "not a pinned tab" };
            }
            mgr.resetPinnedTab(t);
            reset++;
            return { url, reset: true, reason: null };
          });
          return { reset, skipped, tabs };
        },

        async essentialsReplaceUrl(tabUrlsJson) {
          const win = topWindow();
          const mgr = pinnedMgr(win);
          const urls = parseUrls(tabUrlsJson);
          const matched = matchTabsByUrl(win, urls);
          if (matched.length === 0) {
            throw new ExtensionError(
              `no matching tabs in the active window for: ${urls.join(", ")}`
            );
          }
          let replaced = 0;
          let skipped = 0;
          const tabs = matched.map((t) => {
            const url = t.linkedBrowser?.currentURI?.spec ?? null;
            if (!t.pinned) {
              skipped++;
              return { url, replaced: false, reason: "not a pinned tab" };
            }
            mgr.replacePinnedUrlWithCurrent(t);
            replaced++;
            return { url, replaced: true, reason: null };
          });
          return { replaced, skipped, tabs };
        },

        async splitUnsplit() {
          const splitter = topWindow().gZenViewSplitter;
          if (!splitter || typeof splitter.unsplitCurrentView !== "function") {
            throw new Error("gZenViewSplitter.unsplitCurrentView unavailable");
          }
          splitter.unsplitCurrentView();
          return { unsplit: true };
        },

        async splitViewList() {
          // Walk every navigator:browser window's gZenViewSplitter._data,
          // serialize the recursive layoutTree (nsSplitNode / nsSplitLeafNode
          // from ZenViewSplitter.mjs). currentView is an index into _data; -1
          // means no split is focused in that window.
          const serializeTab = (tab) => {
            if (!tab) return null;
            const browser = tab.linkedBrowser;
            return {
              url: browser?.currentURI?.spec ?? null,
              title: tab.label ?? null,
              pinned: !!tab.pinned,
              index: typeof tab._tPos === "number" ? tab._tPos : null,
            };
          };
          const serializeNode = (node, path = "") => {
            if (!node) return null;
            // nsSplitLeafNode has .tab; nsSplitNode has .children + .direction
            if (Array.isArray(node.children)) {
              return {
                type: "node",
                path,
                direction: node.direction ?? null,
                size_in_parent: node.sizeInParent ?? null,
                children: node.children.map((child, i) => serializeNode(child, path ? `${path}.${i}` : `${i}`)),
              };
            }
            return {
              type: "leaf",
              path,
              size_in_parent: node.sizeInParent ?? null,
              tab: serializeTab(node.tab),
            };
          };

          const groups = [];
          const e = Services.wm.getEnumerator("navigator:browser");
          while (e.hasMoreElements()) {
            const w = e.getNext();
            const splitter = w.gZenViewSplitter;
            if (!splitter || !Array.isArray(splitter._data)) continue;
            const currentView = typeof splitter.currentView === "number"
              ? splitter.currentView
              : -1;
            for (let i = 0; i < splitter._data.length; i++) {
              const g = splitter._data[i];
              if (!g) continue;
              groups.push({
                group_id: g.groupId ?? null,
                grid_type: g.gridType ?? null,
                active: i === currentView,
                tabs: Array.isArray(g.tabs) ? g.tabs.map(serializeTab) : [],
                layout: serializeNode(g.layoutTree),
              });
            }
          }
          const active = groups.find((g) => g.active) ?? null;
          return {
            active_group_id: active ? active.group_id : null,
            count: groups.length,
            groups,
          };
        },

        async splitViewAddTab(tabUrlsJson, gridType = "") {
          const urls = parseUrls(tabUrlsJson);
          const win = topWindow();
          const splitter = win.gZenViewSplitter;
          if (!splitter || typeof splitter.splitTabs !== "function") {
            throw new ExtensionError("gZenViewSplitter.splitTabs unavailable");
          }
          const matched = matchTabsByUrl(win, urls);
          if (matched.length === 0) {
            throw new ExtensionError(`no matching tabs in the active window for: ${urls.join(", ")}`);
          }
          splitter.splitTabs(matched, gridType || undefined);
          return { added: matched.length, grid_type: gridType || null };
        },

        async splitViewSetLayout(gridType) {
          const win = topWindow();
          const splitter = win.gZenViewSplitter;
          if (!splitter || !Array.isArray(splitter._data) || typeof splitter.splitTabs !== "function") {
            throw new ExtensionError("gZenViewSplitter unavailable");
          }
          const idx = typeof splitter.currentView === "number" ? splitter.currentView : -1;
          const group = idx >= 0 ? splitter._data[idx] : null;
          if (!group || !Array.isArray(group.tabs) || group.tabs.length < 2) {
            throw new ExtensionError("no active split group");
          }
          splitter.splitTabs(group.tabs, gridType || "grid");
          return { layout: gridType || "grid", group_id: group.groupId ?? null };
        },

        async splitViewResize(path, sizesJson) {
          const win = topWindow();
          const splitter = win.gZenViewSplitter;
          if (!splitter || !Array.isArray(splitter._data) || typeof splitter.applyGridLayout !== "function") {
            throw new ExtensionError("gZenViewSplitter.applyGridLayout unavailable");
          }
          const idx = typeof splitter.currentView === "number" ? splitter.currentView : -1;
          const group = idx >= 0 ? splitter._data[idx] : null;
          if (!group?.layoutTree) throw new ExtensionError("no active split group");

          let sizes;
          try {
            sizes = JSON.parse(sizesJson || "[]").map((n) => Number(n));
          } catch (e) {
            throw new ExtensionError(`invalid sizes JSON: ${String(e?.message ?? e)}`);
          }
          if (!Array.isArray(sizes) || sizes.some((n) => !Number.isFinite(n) || n <= 0)) {
            throw new ExtensionError("sizes must be positive numbers");
          }
          const sum = sizes.reduce((a, b) => a + b, 0);
          if (Math.abs(sum - 100) > 0.5) {
            throw new ExtensionError(`sizes must sum to 100 (got ${sum})`);
          }

          let node = group.layoutTree;
          const parts = String(path || "").trim() ? String(path).split(/[./]/).filter(Boolean) : [];
          for (const part of parts) {
            const i = Number.parseInt(part, 10);
            if (!Array.isArray(node?.children) || !Number.isInteger(i) || i < 0 || i >= node.children.length) {
              throw new ExtensionError(`invalid layout node path: ${path}`);
            }
            node = node.children[i];
          }
          if (!Array.isArray(node?.children)) {
            throw new ExtensionError(`layout node path is a leaf: ${path || "<root>"}`);
          }
          if (node.children.length !== sizes.length) {
            throw new ExtensionError(`size count (${sizes.length}) must match child count (${node.children.length})`);
          }
          const min = Number(splitter.minResizeWidth || 0);
          for (const n of sizes) {
            if (min > 0 && n < min) {
              throw new ExtensionError(`size ${n} below minimum ${min}`);
            }
          }
          const normalized = sizes.map((n) => (n / sum) * 100);
          node.children.forEach((child, i) => {
            child.sizeInParent = normalized[i];
          });
          splitter.applyGridLayout(group.layoutTree);
          return {
            resized: true,
            group_id: group.groupId ?? null,
            path: path || "",
            sizes: normalized,
          };
        },

        async splitViewRearrange(enable) {
          const win = topWindow();
          const splitter = win.gZenViewSplitter;
          if (!splitter || typeof splitter.enableTabRearrangeView !== "function" || typeof splitter.disableTabRearrangeView !== "function") {
            throw new ExtensionError("gZenViewSplitter.rearrangeView unavailable");
          }
          if (enable) {
            splitter.enableTabRearrangeView(false);
          } else {
            splitter.disableTabRearrangeView();
          }
          return { rearranging: !!splitter.rearrangeViewEnabled };
        },

        async shortcutsReset() {
          const win = topWindow();
          const shortcuts = win.ZenKeyboardShortcuts;
          if (!shortcuts || typeof shortcuts.resetAllShortcuts !== "function") {
            throw new ExtensionError("ZenKeyboardShortcuts.resetAllShortcuts unavailable");
          }
          await shortcuts.resetAllShortcuts();
          return { reset: true };
        },

        async shareCan() {
          // Services.zen comes from nsIZenCommonUtils. canShare() returns
          // false on Linux (no native share sheet) and on Windows builds
          // older than 10.0.18956.
          try {
            return { can: !!Services.zen.canShare() };
          } catch (e) {
            return { can: false, reason: String(e?.message ?? e) };
          }
        },

        async share(url, title, text) {
          if (!url) throw new ExtensionError("url required");
          if (typeof Services.zen?.canShare !== "function" ||
              typeof Services.zen?.share !== "function") {
            throw new ExtensionError("Services.zen unavailable (not a Zen build?)");
          }
          if (!Services.zen.canShare()) {
            throw new ExtensionError(
              "native share not supported on this platform (Linux, or Windows < 10.0.18956)"
            );
          }
          let uri;
          try {
            uri = Services.io.newURI(url);
          } catch (e) {
            throw new ExtensionError(`invalid url: ${String(e?.message ?? e)}`);
          }
          // Anchor the share popover. Windows ignores the rect; macOS shows
          // the popover relative to it (Y is bottom-up in window coords, per
          // ZenCommonUtils.mjs's own call site).
          const win = topWindow();
          let x = 0, y = 0, w = 0, h = 0;
          try {
            const rect = win.windowUtils.getBoundsWithoutFlushing(
              win.gBrowser.tabpanels
            );
            if (rect && rect.width && rect.height) {
              const sw = 1, sh = 1;
              x = Math.round(rect.left + rect.width / 2 - sw / 2);
              y = Math.round(win.innerHeight - (rect.top + rect.height / 2 + sh / 2));
              w = sw;
              h = sh;
            }
          } catch (_) { /* fall through with zeros */ }
          try {
            Services.zen.share(uri, title ?? "", text ?? "", x, y, w, h);
          } catch (e) {
            throw new ExtensionError(`share failed: ${String(e?.message ?? e)}`);
          }
          return { opened: true, url };
        },

        async modsList() {
          const mgr = topWindow().gZenMods;
          if (!mgr || typeof mgr.getMods !== "function") {
            throw new Error("gZenMods unavailable");
          }
          const mods = await mgr.getMods();
          return Object.values(mods || {});
        },

        async modsInstall(modId, url) {
          // Wrap the entire body — experiment-API bridge mangles bare
          // Errors into "An unexpected error occurred @undefined:0" if
          // anything throws unexpectedly. The outer catch re-throws with a
          // stage label so the CLI gets something actionable.
          let stage = "init";
          try {
            const win = topWindow();
            const mgr = win.gZenMods;
            if (!mgr || typeof mgr.getMods !== "function" || typeof mgr.updateMods !== "function") {
              throw new ExtensionError("gZenMods install API unavailable");
            }
            // Zen's own gZenMods.requestMod() fetches with `mode: "no-cors"`,
            // leaving the response opaque and unreadable. The chrome window
            // (where gZenMods lives) has a content principal that can do a
            // normal CORS fetch on the theme store, which is what we need.
            // `url` overrides the store URL entirely (forks, dev builds,
            // off-store mods).
            const fetchUrl = url && url.length > 0
              ? url
              : `https://zen-browser.github.io/theme-store/themes/${modId}/theme.json`;

            stage = `fetch ${fetchUrl}`;
            // Use the navigator:browser window's fetch — same caller Zen's
            // own gZenMods uses, no experiment-sandbox global lookup.
            const res = await win.fetch(fetchUrl);
            if (!res.ok) {
              throw new ExtensionError(`HTTP ${res.status}`);
            }
            stage = "parse json";
            const mod = await res.json();
            if (!mod || typeof mod !== "object" || !mod.id) {
              throw new ExtensionError("theme.json missing required 'id' field");
            }

            // Match the marketplace actor's install flow (see
            // ZenModsMarketplaceParent.sys.mjs): enable + register +
            // updateMods (which writes zen-themes.json and downloads files
            // via checkForModChanges → installMod).
            stage = "register";
            mod.enabled = true;
            const mods = await mgr.getMods();
            mods[mod.id] = mod;
            stage = "updateMods";
            await mgr.updateMods(mods);
            return { installed: mod.id, name: mod.name ?? null };
          } catch (e) {
            const msg = e?.message ?? String(e);
            const where = e?.fileName ? ` [${e.fileName}:${e.lineNumber}]` : "";
            try { console.error(`[zenctl] modsInstall failed at ${stage}: ${msg}${where}`, e); } catch (_) {}
            throw new ExtensionError(`modsInstall failed at ${stage}: ${msg}${where}`);
          }
        },

        async modsRemove(modId) {
          const mgr = topWindow().gZenMods;
          if (!mgr || typeof mgr.removeMod !== "function") {
            throw new Error("gZenMods unavailable");
          }
          await mgr.removeMod(modId);
          return { removed: modId };
        },

        async modsEnable(modId) {
          const mgr = topWindow().gZenMods;
          if (!mgr || typeof mgr.enableMod !== "function") {
            throw new Error("gZenMods unavailable");
          }
          await mgr.enableMod(modId);
          return { enabled: modId };
        },

        async modsDisable(modId) {
          const mgr = topWindow().gZenMods;
          if (!mgr || typeof mgr.disableMod !== "function") {
            throw new Error("gZenMods unavailable");
          }
          await mgr.disableMod(modId);
          return { disabled: modId };
        },

        async foldersList() {
          // Enumerate every <zen-folder> element across all navigator:browser
          // windows. nsZenFolder extends MozTabbrowserTabGroup; folders are
          // pinned-tab-area DOM elements (see zen/folders/ZenFolders.mjs).
          const folders = [];
          const e = Services.wm.getEnumerator("navigator:browser");
          while (e.hasMoreElements()) {
            const w = e.getNext();
            if (!w.document) continue;
            const nodes = w.document.querySelectorAll("zen-folder");
            for (const f of nodes) {
              const parentFolder = f.group && f.group.isZenFolder ? f.group : null;
              const tabs = [];
              const items = typeof f.allItems !== "undefined" ? f.allItems : [];
              for (const item of items) {
                if (item.isZenFolder) continue; // nested folder; shown via its own row
                if (item.hasAttribute && item.hasAttribute("zen-empty-tab")) continue;
                const b = item.linkedBrowser;
                tabs.push({
                  url: b?.currentURI?.spec ?? null,
                  title: item.label ?? null,
                  pinned: !!item.pinned,
                });
              }
              folders.push({
                id: f.id || null,
                label: f.label || null,
                icon: f.iconURL || null,
                collapsed: !!f.collapsed,
                parent_id: parentFolder?.id ?? null,
                is_live_folder: !!f.isLiveFolder,
                tabs,
              });
            }
          }
          return { count: folders.length, folders };
        },

        async foldersCreate(label, workspaceId) {
          const win = topWindow();
          const mgr = win.gZenFolders;
          if (!mgr || typeof mgr.createFolder !== "function") {
            throw new ExtensionError("gZenFolders.createFolder unavailable");
          }
          // createFolder() always inserts an empty placeholder tab; passing []
          // produces an empty folder with that placeholder only.
          const folder = mgr.createFolder([], {
            label: label || "New Folder",
            workspaceId: workspaceId || undefined,
          });
          return {
            created: !!folder,
            id: folder?.id ?? null,
            label: folder?.label ?? null,
          };
        },

        async foldersDelete(folderId) {
          if (!folderId) throw new ExtensionError("folder id required");
          const win = topWindow();
          const folder = win.document.getElementById(folderId);
          if (!folder || !folder.isZenFolder) {
            throw new ExtensionError(`folder not found: ${folderId}`);
          }
          await folder.delete();
          return { deleted: folderId };
        },

        async foldersRename(folderId, name) {
          if (!folderId) throw new ExtensionError("folder id required");
          if (!name) throw new ExtensionError("new name required");
          const win = topWindow();
          const folder = win.document.getElementById(folderId);
          if (!folder || !folder.isZenFolder) {
            throw new ExtensionError(`folder not found: ${folderId}`);
          }
          // Mirror the onRenameFinished path: set .name (custom-element setter
          // persists + fires ZenFolderRenamed) instead of triggering the inline
          // UI via folder.rename().
          folder.name = name;
          folder.dispatchEvent(new win.CustomEvent("ZenFolderRenamed", { bubbles: true }));
          return { renamed: folderId, name };
        },

        async foldersAddTab(folderId, tabUrlsJson) {
          if (!folderId) throw new ExtensionError("folder id required");
          const win = topWindow();
          const folder = win.document.getElementById(folderId);
          if (!folder || !folder.isZenFolder) {
            throw new ExtensionError(`folder not found: ${folderId}`);
          }
          if (typeof folder.addTabs !== "function") {
            throw new ExtensionError("folder.addTabs unavailable (Zen API change?)");
          }
          let urls;
          try {
            urls = tabUrlsJson ? JSON.parse(tabUrlsJson) : [];
          } catch (e) {
            throw new ExtensionError(`invalid tab urls JSON: ${String(e?.message ?? e)}`);
          }
          if (!Array.isArray(urls) || urls.length === 0) {
            throw new ExtensionError("at least one tab url required");
          }
          // Walk all browser windows for XUL tabs matching the requested URLs.
          // Mirrors splitViewCreate's lookup pattern.
          const allTabs = [];
          const e = Services.wm.getEnumerator("navigator:browser");
          while (e.hasMoreElements()) {
            const w = e.getNext();
            if (!w.gBrowser?.tabContainer) continue;
            for (const t of w.gBrowser.tabContainer.querySelectorAll("tab")) {
              const u = t.linkedBrowser?.currentURI?.spec;
              if (u && urls.includes(u)) allTabs.push(t);
            }
          }
          if (allTabs.length === 0) {
            throw new ExtensionError(`no matching tabs for: ${urls.join(", ")}`);
          }
          // addTabs() pins them automatically since folders live in the
          // pinned area. matchedUrls reports which ones were actually added.
          folder.addTabs(allTabs);
          const matchedUrls = allTabs.map((t) => t.linkedBrowser?.currentURI?.spec ?? null);
          return { folder_id: folderId, added: matchedUrls.length, urls: matchedUrls };
        },

        async foldersSetIcon(folderId, icon) {
          if (!folderId) throw new ExtensionError("folder id required");
          const win = topWindow();
          const mgr = win.gZenFolders;
          if (!mgr || typeof mgr.setFolderUserIcon !== "function") {
            throw new ExtensionError("gZenFolders.setFolderUserIcon unavailable");
          }
          const folder = win.document.getElementById(folderId);
          if (!folder || !folder.isZenFolder) {
            throw new ExtensionError(`folder not found: ${folderId}`);
          }
          // setFolderUserIcon writes the value verbatim into <image href="...">,
          // which only accepts an image URL or data URI. Mirror Zen's emoji
          // picker (#selectEmoji): anything that isn't already a URL gets
          // wrapped as a base64 SVG containing the glyph. We also accept a
          // bare built-in-icon name like "flask.svg" and resolve it to Zen's
          // selectable-icons chrome URL.
          let resolved = icon || "";
          if (resolved) {
            const isUrl = /^(chrome|resource|moz-extension|https?|data):/i.test(resolved);
            const isBareSvg = /^[a-z0-9-]+\.svg$/i.test(resolved);
            if (isBareSvg) {
              resolved = `chrome://browser/skin/zen-icons/selectable/${resolved}`;
            } else if (!isUrl) {
              const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><text y="28" font-size="28" x="0">${unescape(encodeURIComponent(resolved))}</text></svg>`;
              resolved = `data:image/svg+xml;base64,${win.btoa(svg)}`;
            }
          }
          mgr.setFolderUserIcon(folder, resolved);
          folder.dispatchEvent(new win.CustomEvent("TabGroupUpdate", { bubbles: true }));
          return { folder_id: folderId, icon: folder.iconURL || "" };
        },

        async foldersCreateSubfolder(parentId, label) {
          if (!parentId) throw new ExtensionError("parent folder id required");
          const win = topWindow();
          const mgr = win.gZenFolders;
          if (!mgr || typeof mgr.createFolder !== "function") {
            throw new ExtensionError("gZenFolders.createFolder unavailable");
          }
          const parent = win.document.getElementById(parentId);
          if (!parent || !parent.isZenFolder) {
            throw new ExtensionError(`parent folder not found: ${parentId}`);
          }
          // Mirror nsZenFolder.createSubfolder(): force-expand ancestors so
          // the new folder is visible after creation.
          let cursor = parent;
          while (cursor) {
            cursor.collapsed = false;
            cursor = cursor.group;
          }
          const child = mgr.createFolder([], {
            label: label || "Subfolder",
            insertAfter: parent.groupContainer
              ? parent.groupContainer.lastElementChild
              : undefined,
          });
          return {
            created: !!child,
            id: child?.id ?? null,
            parent_id: parentId,
            label: child?.label ?? null,
          };
        },

        async foldersUnpack(folderId) {
          if (!folderId) throw new ExtensionError("folder id required");
          const win = topWindow();
          const folder = win.document.getElementById(folderId);
          if (!folder || !folder.isZenFolder) {
            throw new ExtensionError(`folder not found: ${folderId}`);
          }
          if (typeof folder.unpackTabs !== "function") {
            throw new ExtensionError("folder.unpackTabs unavailable");
          }
          // unpackTabs() iterates allItems in reverse, ungrouping each tab
          // (or removing the empty placeholder). After it runs, the folder
          // itself is empty; Zen removes empty groups automatically.
          await folder.unpackTabs();
          return { unpacked: folderId };
        },

        async foldersUnload(folderId) {
          if (!folderId) throw new ExtensionError("folder id required");
          const win = topWindow();
          const folder = win.document.getElementById(folderId);
          if (!folder || !folder.isZenFolder) {
            throw new ExtensionError(`folder not found: ${folderId}`);
          }
          if (typeof folder.unloadAllTabs !== "function") {
            throw new ExtensionError("folder.unloadAllTabs unavailable");
          }
          // Pass null event — the handler only inspects event.target for the
          // reset-button branch, so a null event safely falls through to the
          // unload branch.
          folder.unloadAllTabs(null);
          return { unloaded: folderId };
        },

        async foldersMoveToWorkspace(folderId, workspaceId) {
          if (!folderId) throw new ExtensionError("folder id required");
          if (!workspaceId) throw new ExtensionError("workspace id required");
          const win = topWindow();
          const mgr = win.gZenFolders;
          if (!mgr || typeof mgr.changeFolderToSpace !== "function") {
            throw new ExtensionError("gZenFolders.changeFolderToSpace unavailable");
          }
          const folder = win.document.getElementById(folderId);
          if (!folder || !folder.isZenFolder) {
            throw new ExtensionError(`folder not found: ${folderId}`);
          }
          mgr.changeFolderToSpace(folder, workspaceId);
          return { folder_id: folderId, workspace_id: workspaceId };
        },

        async foldersConvertToWorkspace(folderId) {
          if (!folderId) throw new ExtensionError("folder id required");
          const win = topWindow();
          const folder = win.document.getElementById(folderId);
          if (!folder || !folder.isZenFolder) {
            throw new ExtensionError(`folder not found: ${folderId}`);
          }
          const ws = win.gZenWorkspaces;
          if (!ws || typeof ws.createAndSaveWorkspace !== "function" || typeof ws.workspaceElement !== "function") {
            throw new ExtensionError("gZenWorkspaces workspace creation API unavailable");
          }
          if (typeof folder.delete !== "function") {
            throw new ExtensionError("folder.delete unavailable");
          }

          // Mirrors ZenFolders.mjs #convertFolderToSpace(): create a new
          // workspace named after the folder, move every non-empty item into
          // that workspace's pinned-tabs container, delete the old folder shell,
          // and update workspace ids/session state. This private method is not
          // callable from the experiment, so keep this copy source-aligned.
          const currentWorkspace = ws.getActiveWorkspaceFromCache?.() || ws.getActiveWorkspace?.();
          if (!currentWorkspace?.uuid) {
            throw new ExtensionError("active workspace unavailable");
          }
          const selectedTab = Array.from(folder.tabs || []).find((tab) => tab.selected) || null;
          const icon = folder.icon?.querySelector?.("svg .icon image");
          const tabsToMoveCount = Array.from(folder.allItems || []).filter(
            (tab) => !tab.hasAttribute?.("zen-empty-tab")
          ).length;

          const newSpace = await ws.createAndSaveWorkspace(
            folder.label || "Folder",
            icon?.getAttribute?.("href"),
            false,
            currentWorkspace.containerTabId,
            {
              beforeChangeCallback: async (newWorkspace) => {
                await new Promise((resolve, reject) => {
                  win.requestAnimationFrame(() => {
                    (async () => {
                      const workspacePinnedContainer = ws.workspaceElement(
                        newWorkspace.uuid
                      ).pinnedTabsContainer;
                      const tabs = Array.from(folder.allItems || []).filter(
                        (tab) => !tab.hasAttribute?.("zen-empty-tab")
                      );
                      workspacePinnedContainer.append(...tabs);
                      await folder.delete();
                      win.gBrowser.tabContainer._invalidateCachedTabs();
                      if (selectedTab) {
                        selectedTab.setAttribute("zen-workspace-id", newWorkspace.uuid);
                        selectedTab.removeAttribute("folder-active");
                        ws.lastSelectedWorkspaceTabs[newWorkspace.uuid] = selectedTab;
                      }
                    })().then(resolve, reject);
                  });
                });
              },
            }
          );
          if (!newSpace?.uuid) {
            throw new ExtensionError("workspace creation failed");
          }

          for (const tab of win.gBrowser.tabs) {
            if (!tab.hasAttribute("zen-essential")) {
              tab.setAttribute("zen-workspace-id", newSpace.uuid);
              tab.style.opacity = "";
              tab.style.height = "";
            }
            win.gBrowser.TabStateFlusher.flush(tab.linkedBrowser);
            if (ws.lastSelectedWorkspaceTabs[currentWorkspace.uuid] === tab) {
              delete ws.lastSelectedWorkspaceTabs[currentWorkspace.uuid];
            }
          }

          return {
            folder_id: folderId,
            workspace_id: newSpace.uuid,
            name: newSpace.name ?? folder.label ?? "Folder",
            moved_items: tabsToMoveCount,
          };
        },

        async foldersCollapse(folderId, collapsed) {
          if (!folderId) throw new ExtensionError("folder id required");
          const win = topWindow();
          const folder = win.document.getElementById(folderId);
          if (!folder || !folder.isZenFolder) {
            throw new ExtensionError(`folder not found: ${folderId}`);
          }
          folder.collapsed = !!collapsed;
          return { id: folderId, collapsed: !!folder.collapsed };
        },

        async modsPreferences(modId) {
          const mgr = topWindow().gZenMods;
          if (!mgr || typeof mgr.getMods !== "function") {
            throw new Error("gZenMods unavailable");
          }
          const mods = await mgr.getMods();
          const mod = (mods || {})[modId];
          if (!mod) {
            throw new Error("mod not installed: " + modId);
          }
          const preferences =
            typeof mgr.getModPreferences === "function"
              ? await mgr.getModPreferences(mod)
              : null;
          return { id: modId, preferences };
        },

        async liveFoldersList() {
          const mgr = importZenModule("resource:///modules/zen/ZenLiveFoldersManager.sys.mjs", "ZenLiveFoldersManager");
          const folders = [];
          for (const [id, lf] of mgr.liveFolders || []) {
            const folder = mgr.getFolderForLiveFolder?.(lf);
            folders.push({ id, provider: lf.constructor?.type ?? null, type: lf.state?.type ?? null, url: lf.state?.url ?? null, label: folder?.label ?? null, last_fetched: lf.state?.lastFetched ?? null, last_error_id: lf.state?.lastErrorId ?? null });
          }
          return { count: folders.length, folders };
        },

        async liveFoldersCreate(provider, url, label) {
          if (!provider) throw new ExtensionError("provider required");
          const mgr = importZenModule("resource:///modules/zen/ZenLiveFoldersManager.sys.mjs", "ZenLiveFoldersManager");
          const win = topWindow();
          const [kind, subtype] = provider.split(":");
          let ProviderClass;
          let state = { interval: 30 * 60 * 1000, lastFetched: 0, lastErrorId: null };
          let folderLabel = label || "Live Folder";
          let icon = null;
          if (kind === "rss") {
            if (!url) throw new ExtensionError("url required for RSS live folder");
            ProviderClass = importZenModule("resource:///modules/zen/RssLiveFolder.sys.mjs", "nsRssLiveFolderProvider");
            const meta = await ProviderClass.getMetadata(url, win);
            folderLabel = label || meta.label || url;
            icon = meta.icon || win.gZenEmojiPicker?.getSVGURL?.("logo-rss.svg") || null;
            state.url = url;
          } else if (kind === "github") {
            ProviderClass = importZenModule("resource:///modules/zen/GithubLiveFolder.sys.mjs", "nsGithubLiveFolderProvider");
            const type = subtype || "pull-requests";
            if (!["pull-requests", "issues"].includes(type)) throw new ExtensionError("github type must be pull-requests|issues");
            folderLabel = label || (type === "issues" ? "GitHub Issues" : "GitHub Pull Requests");
            icon = "chrome://browser/skin/zen-icons/selectable/logo-github.svg";
            state.type = type;
          } else {
            throw new ExtensionError("provider must be rss|github:pull-requests|github:issues");
          }
          const folder = win.gZenFolders.createFolder([], { label: folderLabel, isLiveFolder: true, collapsed: true });
          if (icon && typeof win.gZenFolders.setFolderUserIcon === "function") win.gZenFolders.setFolderUserIcon(folder, icon);
          const liveFolder = new ProviderClass({ state, manager: mgr, id: folder.id });
          mgr.liveFolders.set(folder.id, liveFolder);
          liveFolder.start();
          mgr.saveState();
          return { created: true, id: folder.id, label: folderLabel, provider: kind, type: state.type ?? null, url: state.url ?? null };
        },

        async liveFoldersDelete(folderId) {
          if (!folderId) throw new ExtensionError("folder id required");
          const mgr = importZenModule("resource:///modules/zen/ZenLiveFoldersManager.sys.mjs", "ZenLiveFoldersManager");
          const deleted = !!mgr.deleteFolder(folderId, true);
          return { deleted, id: folderId };
        },

        async liveFoldersRefresh(folderId) {
          if (!folderId) throw new ExtensionError("folder id required");
          const mgr = importZenModule("resource:///modules/zen/ZenLiveFoldersManager.sys.mjs", "ZenLiveFoldersManager");
          const liveFolder = mgr.getFolder(folderId);
          if (!liveFolder) throw new ExtensionError(`live folder not found: ${folderId}`);
          const items = await liveFolder.refresh();
          return { refreshed: true, id: folderId, items: Array.isArray(items) ? items.length : null, result: typeof items === "string" ? items : null };
        },

        async liveFoldersPause(folderId) {
          if (!folderId) throw new ExtensionError("folder id required");
          const mgr = importZenModule("resource:///modules/zen/ZenLiveFoldersManager.sys.mjs", "ZenLiveFoldersManager");
          const liveFolder = mgr.getFolder(folderId);
          if (!liveFolder) throw new ExtensionError(`live folder not found: ${folderId}`);
          if (typeof liveFolder.stop !== "function") throw new ExtensionError("live folder stop unavailable");
          liveFolder.stop();
          return { paused: true, id: folderId };
        },

        async liveFoldersResume(folderId) {
          if (!folderId) throw new ExtensionError("folder id required");
          const mgr = importZenModule("resource:///modules/zen/ZenLiveFoldersManager.sys.mjs", "ZenLiveFoldersManager");
          const liveFolder = mgr.getFolder(folderId);
          if (!liveFolder) throw new ExtensionError(`live folder not found: ${folderId}`);
          if (typeof liveFolder.start !== "function") throw new ExtensionError("live folder start unavailable");
          liveFolder.start();
          return { resumed: true, id: folderId };
        },

        async boostsList() {
          const mgr = importZenModule("resource:///modules/zen/boosts/ZenBoostsManager.sys.mjs", "gZenBoostsManager");
          const domains = [];
          for (const [domain, entry] of mgr.registeredDomains || []) {
            domains.push({ domain, active: entry.activeBoost, boosts: Array.from(entry.boostEntries || []).map(([id, boostEntry]) => ({ id, boost_data: boostEntry.boostData ?? boostEntry })) });
          }
          return { count: domains.length, domains };
        },

        async boostsCreate(domain) {
          if (!domain) throw new ExtensionError("domain required");
          const mgr = importZenModule("resource:///modules/zen/boosts/ZenBoostsManager.sys.mjs", "gZenBoostsManager");
          const boost = mgr.createNewBoost(domain);
          if (typeof mgr.saveBoostToStore === "function") mgr.saveBoostToStore(boost);
          return { created: !!boost, domain: boost?.domain ?? domain, id: boost?.id ?? null };
        },

        async boostsDelete(domain, id) {
          if (!domain || !id) throw new ExtensionError("domain + id required");
          const mgr = importZenModule("resource:///modules/zen/boosts/ZenBoostsManager.sys.mjs", "gZenBoostsManager");
          mgr.deleteBoost({ domain, id });
          return { deleted: true, domain, id };
        },

        async boostsActivate(domain, id) {
          if (!domain || !id) throw new ExtensionError("domain + id required");
          const mgr = importZenModule("resource:///modules/zen/boosts/ZenBoostsManager.sys.mjs", "gZenBoostsManager");
          mgr.makeBoostActiveForDomain(domain, id);
          if (typeof mgr.saveBoostToStore === "function") mgr.saveBoostToStore(null);
          return { active: id, domain };
        },

        async boostsToggle(domain, id) {
          if (!domain || !id) throw new ExtensionError("domain + id required");
          const mgr = importZenModule("resource:///modules/zen/boosts/ZenBoostsManager.sys.mjs", "gZenBoostsManager");
          mgr.toggleBoostActiveForDomain(domain, id);
          if (typeof mgr.saveBoostToStore === "function") mgr.saveBoostToStore(null);
          return { toggled: id, domain, active: mgr.getActiveBoostId(domain) };
        },

        async boostsUpdate(domain, id, dataJson) {
          if (!domain || !id) throw new ExtensionError("domain + id required");
          if (!dataJson) throw new ExtensionError("dataJson required");
          const mgr = importZenModule("resource:///modules/zen/boosts/ZenBoostsManager.sys.mjs", "gZenBoostsManager");
          const boost = mgr.loadBoostFromStore(domain, id);
          if (!boost?.boostEntry?.boostData) throw new ExtensionError("boost not found");
          const updateFields = JSON.parse(dataJson);
          Object.assign(boost.boostEntry.boostData, updateFields);
          if (typeof mgr.saveBoostToStore === "function") mgr.saveBoostToStore(boost);
          return { updated: true, domain, id, boost_data: boost.boostEntry.boostData };
        },

        /**
         * Return workspace_id for every XUL tab across all browser windows.
         * Uses Firefox's TabManager to map XUL `<tab>` elements to
         * WebExtension tab IDs, enabling cross-workspace tab enrichment.
         * Falls back to URL+index matching if TabManager is unavailable.
         *
         * Returns: { tabs: [{ tab_id, workspace_id, url, window_id, index }] }
         */
        async getTabWorkspaces() {
          let getId = null;
          try {
            const { TabManager } = ChromeUtils.importESModule(
              "resource://gre/modules/ExtensionParent.sys.mjs"
            );
            getId = (tab) => TabManager.getId?.(tab) ?? null;
          } catch (_) {
            // TabManager unavailable — caller falls back to URL+index match.
          }

          // Build workspace UUID → name map from the cache.
          const wsNameMap = new Map();
          const win = Services.wm.getMostRecentWindow("navigator:browser");
          if (win?.gZenWorkspaces) {
            const cache = Array.isArray(win.gZenWorkspaces._workspaceCache)
              ? win.gZenWorkspaces._workspaceCache
              : [];
            for (const ws of cache) {
              wsNameMap.set(ws.uuid, ws.name ?? "");
            }
          }

          const tabs = [];
          const e = Services.wm.getEnumerator("navigator:browser");
          while (e.hasMoreElements()) {
            const w = e.getNext();
            if (!w.gBrowser?.tabContainer) continue;
            for (const tab of w.gBrowser.tabContainer.querySelectorAll("tab")) {
              const browser = tab.linkedBrowser;
              if (!browser) continue;
              const wsId = tab.getAttribute("zen-workspace-id") || "";
              let wsName = wsNameMap.get(wsId);
              if (!wsName) wsName = wsId.length > 8 ? wsId.substring(0, 8) : wsId;
              tabs.push({
                tab_id: getId ? getId(tab) : null,
                workspace_id: wsId,
                workspace_name: wsName,
                url: browser.currentURI?.spec || null,
                window_id: w.windowUtils?.outerWindowID ?? null,
                index: tab._tPos ?? null,
              });
            }
          }
          return { count: tabs.length, tabs };
        },
      },
    };
  }
};
