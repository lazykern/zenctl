/**
 * Tests: meta, prefs, find, search, data handlers.
 */
import { describe, it, expect, beforeAll, vi } from "vitest";
import { getBrowser } from "../setup.js";

let handlers = {};
beforeAll(async () => {
  handlers = (await import("../../src/background.js")).handlers;
});

function mockActiveTab(b) {
  b.tabs.query.mockResolvedValue([{ id: 1, url: "https://x.com", active: true }]);
  b.windows.getAll.mockResolvedValue([{ id: 1, type: "normal", tabs: [{ id: 1, active: true, url: "https://x.com" }] }]);
}

// ═══════════════════════════════════════════════════════════════════════════
// EXT RELOAD / DEBUG
// ═══════════════════════════════════════════════════════════════════════════

describe("ext_reload", () => {
  it("returns reloading flag and schedules reload", async () => {
    vi.useFakeTimers();
    const b = getBrowser();
    const promise = handlers.ext_reload();
    // setTimeout hasn't fired yet
    expect(b.runtime.reload).not.toHaveBeenCalled();
    const result = await promise;
    expect(result.reloading).toBe(true);
    // advance time
    vi.advanceTimersByTime(50);
    expect(b.runtime.reload).toHaveBeenCalled();
    vi.useRealTimers();
  });
});

describe("ext_debug", () => {
  it("returns current debug status when no enabled flag", async () => {
    const result = await handlers.ext_debug({});
    expect(typeof result.enabled).toBe("boolean");
  });
  it("sets debug to true", async () => {
    const b = getBrowser();
    const result = await handlers.ext_debug({ enabled: true });
    expect(b.storage.local.set).toHaveBeenCalledWith({ "zenctl_debug": true });
    expect(result.enabled).toBe(true);
  });
  it("sets debug to false", async () => {
    const b = getBrowser();
    b.storage.local.set.mockResolvedValue(undefined);
    const result = await handlers.ext_debug({ enabled: false });
    expect(b.storage.local.set).toHaveBeenCalledWith({ "zenctl_debug": false });
    expect(result.enabled).toBe(false);
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// PREFS
// ═══════════════════════════════════════════════════════════════════════════

describe("prefs_get", () => {
  it("throws when name missing", async () => {
    await expect(handlers.prefs_get({})).rejects.toThrow("name required");
  });
  it("gets pref value", async () => {
    const b = getBrowser();
    b.zenPrefs.getPref.mockResolvedValue("dark");
    const result = await handlers.prefs_get({ name: "zen.theme.background" });
    expect(result).toBe("dark");
  });
});

describe("prefs_set", () => {
  it("throws when name or value missing", async () => {
    await expect(handlers.prefs_set({ name: "test" })).rejects.toThrow("name + value required");
  });
  it("sets pref value", async () => {
    const b = getBrowser();
    b.zenPrefs.setPref.mockResolvedValue(true);
    const result = await handlers.prefs_set({ name: "zen.view.compact", value: true });
    expect(b.zenPrefs.setPref).toHaveBeenCalledWith("zen.view.compact", true);
    expect(result).toBe(true);
  });
});

describe("prefs_clear", () => {
  it("throws when name missing", async () => {
    await expect(handlers.prefs_clear({})).rejects.toThrow("name required");
  });
  it("clears a pref", async () => {
    const b = getBrowser();
    await handlers.prefs_clear({ name: "zen.ui.test" });
    expect(b.zenPrefs.clearPref).toHaveBeenCalledWith("zen.ui.test");
  });
});

describe("prefs_list", () => {
  it("lists prefs with default prefix", async () => {
    const b = getBrowser();
    b.zenPrefs.listPrefs.mockResolvedValue([{ name: "zen.view.compact", value: false }]);
    const result = await handlers.prefs_list({});
    expect(b.zenPrefs.listPrefs).toHaveBeenCalledWith("zen.");
    expect(result).toHaveLength(1);
  });
  it("lists prefs with custom prefix", async () => {
    const b = getBrowser();
    await handlers.prefs_list({ prefix: "browser." });
    expect(b.zenPrefs.listPrefs).toHaveBeenCalledWith("browser.");
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// BROWSING DATA
// ═══════════════════════════════════════════════════════════════════════════

describe("data_clear", () => {
  it("throws when no types selected", async () => {
    await expect(handlers.data_clear({})).rejects.toThrow("no data types selected");
  });
  it("throws when unknown type selector", async () => {
    // Only allowed keys get through; empty means no types
    await expect(handlers.data_clear({ types: { invalid: true } })).rejects.toThrow("no data types selected");
  });
  it("clears selected types", async () => {
    const b = getBrowser();
    const result = await handlers.data_clear({
      since: 3600,
      types: { cache: true, history: true, downloads: true },
    });
    expect(b.browsingData.remove).toHaveBeenCalledWith(
      { since: 3600 },
      { cache: true, history: true, downloads: true },
    );
    expect(result.cleared.sort()).toEqual(["cache", "downloads", "history"]);
    expect(result.since).toBe(3600);
  });
  it("ignores unknown keys", async () => {
    const b = getBrowser();
    await handlers.data_clear({ types: { cache: true, badThing: true } });
    expect(b.browsingData.remove).toHaveBeenCalledWith(
      { since: 0 },
      { cache: true },
    );
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// FIND IN PAGE
// ═══════════════════════════════════════════════════════════════════════════

describe("find_in_page", () => {
  it("throws when query missing", async () => {
    await expect(handlers.find_in_page({ active: true })).rejects.toThrow("query required");
  });
  it("finds and highlights", async () => {
    const b = getBrowser();
    mockActiveTab(b);
    b.find.find.mockResolvedValue({ count: 3 });
    const result = await handlers.find_in_page({ active: true, query: "zen" });
    expect(b.find.find).toHaveBeenCalledWith("zen", { tabId: 1 });
    expect(b.find.highlightResults).toHaveBeenCalledWith({ tabId: 1 });
    expect(result.count).toBe(3);
  });
  it("passes case_sensitive and entire_word", async () => {
    const b = getBrowser();
    mockActiveTab(b);
    b.find.find.mockResolvedValue({ count: 1 });
    await handlers.find_in_page({ active: true, query: "Zen", case_sensitive: true, entire_word: true });
    expect(b.find.find).toHaveBeenCalledWith("Zen", { tabId: 1, caseSensitive: true, entireWord: true });
  });
  it("does not highlight if count is 0", async () => {
    const b = getBrowser();
    mockActiveTab(b);
    b.find.find.mockResolvedValue({ count: 0 });
    await handlers.find_in_page({ active: true, query: "nothing" });
    expect(b.find.highlightResults).not.toHaveBeenCalled();
  });
});

describe("find_clear", () => {
  it("clears highlighting", async () => {
    const b = getBrowser();
    const result = await handlers.find_clear();
    expect(b.find.removeHighlighting).toHaveBeenCalled();
    expect(result.cleared).toBe(true);
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// SEARCH
// ═══════════════════════════════════════════════════════════════════════════

describe("search_list", () => {
  it("returns search engines", async () => {
    const b = getBrowser();
    b.search.get.mockResolvedValue([{ name: "Google" }]);
    const result = await handlers.search_list();
    expect(result).toHaveLength(1);
  });
});

describe("search_query", () => {
  it("throws when query missing", async () => {
    await expect(handlers.search_query({})).rejects.toThrow("query required");
  });
  it("executes search with engine", async () => {
    const b = getBrowser();
    mockActiveTab(b);
    await handlers.search_query({ query: "cats", engine: "DuckDuckGo" });
    expect(b.search.search).toHaveBeenCalledWith({ query: "cats", engine: "DuckDuckGo", tabId: 1 });
  });
  it("executes search without engine (uses default)", async () => {
    const b = getBrowser();
    mockActiveTab(b);
    const result = await handlers.search_query({ query: "dogs" });
    expect(b.search.search).toHaveBeenCalledWith({ query: "dogs", tabId: 1 });
    expect(result.query).toBe("dogs");
  });
});
