/**
 * WebExtension browser.* API mock factory for vitest.
 *
 * Returns a full browser object where every API is a vi.fn() spy.
 * Tests configure return values per-call:
 *
 *   const browser = createBrowserMock();
 *   browser.tabs.query.mockResolvedValue([{ id: 1, url: "about:blank", title: "" }]);
 *
 * Simulates Firefox WebExtension APIs used by zenctl's background.js.
 * Not exhaustive — add methods as needed by new tests.
 */
import { vi } from "vitest";

// ── Helpers ──────────────────────────────────────────────────────────────

function fn(name) {
  return vi.fn().mockName(name);
}

function eventTarget() {
  const listeners = new Set();
  return {
    addListener(fn) {
      listeners.add(fn);
    },
    removeListener(fn) {
      listeners.delete(fn);
    },
    hasListener(fn) {
      return listeners.has(fn);
    },
    // Test helper: fire the event
    _emit(...args) {
      for (const fn of listeners) fn(...args);
    },
    _clear() {
      listeners.clear();
    },
  };
}

// ── Factory ──────────────────────────────────────────────────────────────

export function createBrowserMock() {
  const browser = {
    // ── Runtime ──────────────────────────────────────────────────
    runtime: {
      connectNative: fn("runtime.connectNative").mockReturnValue({
        onMessage: eventTarget(),
        onDisconnect: eventTarget(),
        postMessage: fn("nativePort.postMessage"),
        disconnect: fn("nativePort.disconnect"),
      }),
      getManifest: fn("runtime.getManifest").mockReturnValue({
        version: "0.1.0",
      }),
      getURL: fn("runtime.getURL").mockImplementation((path) =>
        `moz-extension://fake/${path}`,
      ),
      lastError: null,
      reload: fn("runtime.reload"),
      onConnect: eventTarget(),
      onMessage: eventTarget(),
      onConnectExternal: eventTarget(),
    },

    // ── Storage ──────────────────────────────────────────────────
    storage: {
      local: {
        get: fn("storage.local.get").mockResolvedValue({}),
        set: fn("storage.local.set").mockResolvedValue(undefined),
      },
      onChanged: eventTarget(),
    },

    // ── Tabs ─────────────────────────────────────────────────────
    tabs: {
      query: fn("tabs.query").mockResolvedValue([]),
      get: fn("tabs.get").mockRejectedValue(new Error("tab not found")),
      create: fn("tabs.create").mockRejectedValue(new Error("no window")),
      remove: fn("tabs.remove").mockResolvedValue(undefined),
      update: fn("tabs.update").mockImplementation((_id, changes) =>
        Promise.resolve({ id: _id, ...changes }),
      ),
      reload: fn("tabs.reload").mockResolvedValue(undefined),
      duplicate: fn("tabs.duplicate").mockResolvedValue({ id: 999 }),
      discard: fn("tabs.discard").mockResolvedValue(undefined),
      captureTab: fn("tabs.captureTab").mockResolvedValue("data:image/png;base64,"),
      setZoom: fn("tabs.setZoom").mockResolvedValue(undefined),
      getZoom: fn("tabs.getZoom").mockResolvedValue(1),
      toggleReaderMode: fn("tabs.toggleReaderMode").mockResolvedValue(undefined),
      goBack: fn("tabs.goBack").mockResolvedValue(undefined),
      goForward: fn("tabs.goForward").mockResolvedValue(undefined),
      move: fn("tabs.move").mockImplementation((_ids, _props) =>
        Promise.resolve(Array.isArray(_ids) ? _ids.map((id) => ({ id, index: _props.index })) : [{ id: _ids, index: _props.index }]),
      ),
      executeScript: fn("tabs.executeScript").mockRejectedValue(
        new Error("no tab"),
      ),
      group: fn("tabs.group"),
      ungroup: fn("tabs.ungroup"),
      onCreated: eventTarget(),
      onRemoved: eventTarget(),
      onActivated: eventTarget(),
      onUpdated: eventTarget(),
    },

    // ── Windows ──────────────────────────────────────────────────
    windows: {
      getAll: fn("windows.getAll").mockResolvedValue([]),
      get: fn("windows.get").mockResolvedValue({ id: 1, state: "normal" }),
      create: fn("windows.create").mockResolvedValue({ id: 99, tabs: [{ id: 100 }] }),
      update: fn("windows.update").mockImplementation((_id, _changes) =>
        Promise.resolve({ id: _id, ..._changes }),
      ),
      remove: fn("windows.remove").mockResolvedValue(undefined),
      onCreated: eventTarget(),
      onRemoved: eventTarget(),
      onFocusChanged: eventTarget(),
    },

    // ── Bookmarks ────────────────────────────────────────────────
    bookmarks: {
      getTree: fn("bookmarks.getTree").mockResolvedValue([]),
      getSubTree: fn("bookmarks.getSubTree").mockResolvedValue([]),
      create: fn("bookmarks.create").mockResolvedValue({ id: "bm1", title: "", url: "" }),
      update: fn("bookmarks.update").mockImplementation((_id, _changes) =>
        Promise.resolve({ id: _id, ..._changes }),
      ),
      remove: fn("bookmarks.remove").mockResolvedValue(undefined),
      removeTree: fn("bookmarks.removeTree").mockResolvedValue(undefined),
      move: fn("bookmarks.move").mockImplementation((_id, _dest) =>
        Promise.resolve({ id: _id, ..._dest }),
      ),
      search: fn("bookmarks.search").mockResolvedValue([]),
    },

    // ── Sessions ─────────────────────────────────────────────────
    sessions: {
      getRecentlyClosed: fn("sessions.getRecentlyClosed").mockResolvedValue([]),
      restore: fn("sessions.restore").mockResolvedValue({}),
    },

    // ── History ──────────────────────────────────────────────────
    history: {
      search: fn("history.search").mockResolvedValue([]),
      deleteUrl: fn("history.deleteUrl").mockResolvedValue(undefined),
      addUrl: fn("history.addUrl").mockResolvedValue(undefined),
      getVisits: fn("history.getVisits").mockResolvedValue([]),
    },

    // ── Downloads ────────────────────────────────────────────────
    downloads: {
      search: fn("downloads.search").mockResolvedValue([]),
      cancel: fn("downloads.cancel").mockResolvedValue(undefined),
      download: fn("downloads.download").mockResolvedValue(42),
      pause: fn("downloads.pause").mockResolvedValue(undefined),
      resume: fn("downloads.resume").mockResolvedValue(undefined),
    },

    // ── Cookies ──────────────────────────────────────────────────
    cookies: {
      get: fn("cookies.get").mockResolvedValue(null),
      set: fn("cookies.set").mockImplementation((_d) =>
        Promise.resolve({ ..._d, value: String(_d.value) }),
      ),
      remove: fn("cookies.remove").mockResolvedValue({ url: "", name: "", storeId: "0" }),
    },

    // ── Contextual Identities (containers) ───────────────────────
    contextualIdentities: {
      query: fn("contextualIdentities.query").mockResolvedValue([]),
      create: fn("contextualIdentities.create").mockImplementation((_d) =>
        Promise.resolve({ cookieStoreId: "firefox-container-99", ..._d }),
      ),
      update: fn("contextualIdentities.update").mockImplementation((_id, _d) =>
        Promise.resolve({ cookieStoreId: _id, ..._d }),
      ),
      remove: fn("contextualIdentities.remove").mockResolvedValue({ cookieStoreId: "" }),
    },

    // ── Find ─────────────────────────────────────────────────────
    find: {
      find: fn("find.find").mockResolvedValue({ count: 0 }),
      highlightResults: fn("find.highlightResults").mockResolvedValue(undefined),
      removeHighlighting: fn("find.removeHighlighting").mockResolvedValue(undefined),
    },

    // ── Search ───────────────────────────────────────────────────
    search: {
      get: fn("search.get").mockResolvedValue([]),
      search: fn("search.search").mockResolvedValue(undefined),
    },

    // ── Browsing Data ────────────────────────────────────────────
    browsingData: {
      remove: fn("browsingData.remove").mockResolvedValue(undefined),
    },

    // ── Web Navigation ───────────────────────────────────────────
    webNavigation: {
      getAllFrames: fn("webNavigation.getAllFrames").mockResolvedValue([]),
    },

    // ── Management ───────────────────────────────────────────────
    management: {
      uninstallSelf: fn("management.uninstallSelf").mockResolvedValue(undefined),
    },

    // ── Experiment APIs (zenChrome) ─────────────────────────────
    zenChrome: {
      compactToggle: fn("zenChrome.compactToggle"),
      compactSet: fn("zenChrome.compactSet"),
      compactHide: fn("zenChrome.compactHide"),
      workspaceSwitch: fn("zenChrome.workspaceSwitch"),
      workspacesList: fn("zenChrome.workspacesList"),
      workspaceUnload: fn("zenChrome.workspaceUnload"),
      workspaceCreate: fn("zenChrome.workspaceCreate"),
      workspaceRemove: fn("zenChrome.workspaceRemove"),
      workspaceRename: fn("zenChrome.workspaceRename"),
      workspaceSetIcon: fn("zenChrome.workspaceSetIcon"),
      workspaceSetContainer: fn("zenChrome.workspaceSetContainer"),
      workspaceReorder: fn("zenChrome.workspaceReorder"),
      workspaceMoveTab: fn("zenChrome.workspaceMoveTab"),
      glanceClose: fn("zenChrome.glanceClose"),
      glanceExpand: fn("zenChrome.glanceExpand"),
      glanceList: fn("zenChrome.glanceList"),
      glanceCloseAll: fn("zenChrome.glanceCloseAll"),
      glanceOpen: fn("zenChrome.glanceOpen"),
      splitViewCreate: fn("zenChrome.splitViewCreate"),
      splitUnsplit: fn("zenChrome.splitUnsplit"),
      splitViewList: fn("zenChrome.splitViewList"),
      splitViewAddTab: fn("zenChrome.splitViewAddTab"),
      splitViewRearrange: fn("zenChrome.splitViewRearrange"),
      splitViewResize: fn("zenChrome.splitViewResize"),
      splitViewSetLayout: fn("zenChrome.splitViewSetLayout"),
      urlbarSearch: fn("zenChrome.urlbarSearch"),
      urlbarClose: fn("zenChrome.urlbarClose"),
      urlbarActionsList: fn("zenChrome.urlbarActionsList"),
      urlbarActionsRun: fn("zenChrome.urlbarActionsRun"),
      share: fn("zenChrome.share"),
      shareCan: fn("zenChrome.shareCan"),
      essentialsList: fn("zenChrome.essentialsList"),
      essentialsAdd: fn("zenChrome.essentialsAdd"),
      essentialsRemove: fn("zenChrome.essentialsRemove"),
      essentialsReplaceUrl: fn("zenChrome.essentialsReplaceUrl"),
      essentialsReset: fn("zenChrome.essentialsReset"),
      modsList: fn("zenChrome.modsList"),
      modsInstall: fn("zenChrome.modsInstall"),
      modsRemove: fn("zenChrome.modsRemove"),
      modsEnable: fn("zenChrome.modsEnable"),
      modsDisable: fn("zenChrome.modsDisable"),
      modsPreferences: fn("zenChrome.modsPreferences"),
      boostsList: fn("zenChrome.boostsList"),
      boostsCreate: fn("zenChrome.boostsCreate"),
      boostsDelete: fn("zenChrome.boostsDelete"),
      boostsToggle: fn("zenChrome.boostsToggle"),
      boostsUpdate: fn("zenChrome.boostsUpdate"),
      boostsActivate: fn("zenChrome.boostsActivate"),
      foldersList: fn("zenChrome.foldersList"),
      foldersCreate: fn("zenChrome.foldersCreate"),
      foldersDelete: fn("zenChrome.foldersDelete"),
      foldersRename: fn("zenChrome.foldersRename"),
      foldersSetIcon: fn("zenChrome.foldersSetIcon"),
      foldersAddTab: fn("zenChrome.foldersAddTab"),
      foldersCreateSubfolder: fn("zenChrome.foldersCreateSubfolder"),
      foldersUnload: fn("zenChrome.foldersUnload"),
      foldersUnpack: fn("zenChrome.foldersUnpack"),
      foldersCollapse: fn("zenChrome.foldersCollapse"),
      foldersMoveToWorkspace: fn("zenChrome.foldersMoveToWorkspace"),
      foldersConvertToWorkspace: fn("zenChrome.foldersConvertToWorkspace"),
      liveFoldersList: fn("zenChrome.liveFoldersList"),
      liveFoldersCreate: fn("zenChrome.liveFoldersCreate"),
      liveFoldersDelete: fn("zenChrome.liveFoldersDelete"),
      liveFoldersPause: fn("zenChrome.liveFoldersPause"),
      liveFoldersRefresh: fn("zenChrome.liveFoldersRefresh"),
      liveFoldersResume: fn("zenChrome.liveFoldersResume"),
      getTabWorkspaces: fn("zenChrome.getTabWorkspaces"),
      windowSyncForce: fn("zenChrome.windowSyncForce"),
      shortcutsReset: fn("zenChrome.shortcutsReset"),
    },

    // ── Experiment APIs (zenPrefs) ──────────────────────────────
    zenPrefs: {
      getPref: fn("zenPrefs.getPref"),
      setPref: fn("zenPrefs.setPref"),
      clearPref: fn("zenPrefs.clearPref"),
      listPrefs: fn("zenPrefs.listPrefs"),
    },
  };

  return browser;
}

// ── Default mock installed on globalThis ─────────────────────────────────

const defaultBrowser = createBrowserMock();
export default defaultBrowser;
