/**
 * Tests: helper functions exported from background.js.
 */
import { describe, it, expect, beforeAll, vi } from "vitest";
import { getBrowser } from "./setup.js";

let helpers = {};

beforeAll(async () => {
  const mod = await import("../src/background.js");
  // Grab all non-handler exports
  helpers = Object.fromEntries(
    Object.entries(mod).filter(([k]) => k !== "handlers" && k !== "default")
  );
});

// ═══════════════════════════════════════════════════════════════════════════
// computeFingerprint
// ═══════════════════════════════════════════════════════════════════════════

describe("computeFingerprint", () => {
  it("returns a hex string", async () => {
    const fp = await helpers.computeFingerprint();
    expect(typeof fp).toBe("string");
    expect(fp.length).toBeGreaterThanOrEqual(12);
    expect(/^[0-9a-f]+$/.test(fp)).toBe(true);
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// filterTabs
// ═══════════════════════════════════════════════════════════════════════════

describe("filterTabs", () => {
  const tabs = [
    { id: 1, title: "Zen Browser", url: "https://zen-browser.app", windowId: 1, index: 0, active: true },
    { id: 2, title: "Firefox", url: "https://mozilla.org", windowId: 1, index: 1, active: false },
    { id: 3, title: "GitHub", url: "https://github.com", windowId: 2, index: 0, active: true },
  ];

  it("filters by tab_id", () => {
    expect(helpers.filterTabs(tabs, { tab_id: 2 })).toHaveLength(1);
    expect(helpers.filterTabs(tabs, { tab_id: 999 })).toHaveLength(0);
  });

  it("filters by url_contains", () => {
    expect(helpers.filterTabs(tabs, { url_contains: "github" })).toHaveLength(1);
  });

  it("filters by title_contains", () => {
    expect(helpers.filterTabs(tabs, { title_contains: "Zen" })).toHaveLength(1);
  });

  it("filters by active", () => {
    expect(helpers.filterTabs(tabs, { active: true })).toHaveLength(2);
  });

  it("filters by tab_index", () => {
    expect(helpers.filterTabs(tabs, { tab_index: 0 })).toHaveLength(2);
    expect(helpers.filterTabs(tabs, { tab_index: 1 })).toHaveLength(1);
  });

  it("filters by window_id", () => {
    expect(helpers.filterTabs(tabs, { window_id: 2 })).toHaveLength(1);
  });

  it("combines multiple filters", () => {
    const result = helpers.filterTabs(tabs, {
      window_id: 1, active: true, title_contains: "Zen",
    });
    expect(result).toHaveLength(1);
    expect(result[0].id).toBe(1);
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// normalizePageRef
// ═══════════════════════════════════════════════════════════════════════════

describe("normalizePageRef", () => {
  it("passes through params without ref", () => {
    const out = helpers.normalizePageRef({ selector: "#x" });
    expect(out).toEqual({ selector: "#x" });
  });

  it("converts ref to frame_index", () => {
    const out = helpers.normalizePageRef({ selector: "a", ref: "f3:e7" });
    expect(out.frame_index).toBe(3);
  });

  it("throws on invalid ref", () => {
    expect(() => helpers.normalizePageRef({ ref: "bad" })).toThrow("invalid ref");
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// requireChromeApi / requirePrefsApi
// ═══════════════════════════════════════════════════════════════════════════

describe("requireChromeApi", () => {
  it("does not throw when zenChrome.compactToggle exists", () => {
    expect(() => helpers.requireChromeApi()).not.toThrow();
  });

  it("throws when zenChrome is missing", () => {
    const saved = globalThis.browser.zenChrome;
    globalThis.browser.zenChrome = undefined;
    expect(() => helpers.requireChromeApi()).toThrow("browser.zenChrome missing");
    globalThis.browser.zenChrome = saved;
  });

  it("throws when compactToggle is missing", () => {
    const saved = globalThis.browser.zenChrome;
    globalThis.browser.zenChrome = {};
    expect(() => helpers.requireChromeApi()).toThrow("browser.zenChrome missing");
    globalThis.browser.zenChrome = saved;
  });
});

describe("requirePrefsApi", () => {
  it("does not throw when zenPrefs.getPref exists", () => {
    // setup has zenPrefs with getPref etc.
    expect(() => helpers.requirePrefsApi()).not.toThrow();
  });

  it("throws when zenPrefs is missing", () => {
    const saved = globalThis.browser.zenPrefs;
    globalThis.browser.zenPrefs = undefined;
    expect(() => helpers.requirePrefsApi()).toThrow("browser.zenPrefs missing");
    globalThis.browser.zenPrefs = saved;
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// resolveTabUrls
// ═══════════════════════════════════════════════════════════════════════════

describe("resolveTabUrls", () => {
  it("returns urls directly when given", async () => {
    const result = await helpers.resolveTabUrls({ urls: ["https://a.com", "https://b.com"] });
    expect(result).toEqual(["https://a.com", "https://b.com"]);
  });

  it("resolves tab_ids to urls", async () => {
    const b = getBrowser();
    b.tabs.get.mockResolvedValueOnce({ id: 1, url: "https://tab1.com" });
    b.tabs.get.mockResolvedValueOnce({ id: 2, url: "https://tab2.com" });
    const result = await helpers.resolveTabUrls({ tab_ids: [1, 2] });
    expect(result).toEqual(["https://tab1.com", "https://tab2.com"]);
  });

  it("combines urls and tab_ids", async () => {
    const b = getBrowser();
    b.tabs.get.mockResolvedValue({ id: 3, url: "https://tab3.com" });
    const result = await helpers.resolveTabUrls({
      urls: ["https://direct.com"],
      tab_ids: [3],
    });
    expect(result).toEqual(["https://direct.com", "https://tab3.com"]);
  });

  it("skips tabs with no url", async () => {
    const b = getBrowser();
    b.tabs.get.mockResolvedValue({ id: 4 }); // no url
    const result = await helpers.resolveTabUrls({ tab_ids: [4] });
    expect(result).toEqual([]);
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// targetTabId
// ═══════════════════════════════════════════════════════════════════════════

describe("targetTabId", () => {
  it("returns explicit tab_id", async () => {
    expect(await helpers.targetTabId({ tab_id: 42 })).toBe(42);
  });

  it("returns tab_id from target wrapper", async () => {
    expect(await helpers.targetTabId({ target: { tab_id: 7 } })).toBe(7);
  });

  it("resolves by active + current window", async () => {
    const b = getBrowser();
    b.tabs.query.mockResolvedValue([{ id: 1, active: true, url: "https://x.com", windowId: 1 }]);
    expect(await helpers.targetTabId({ active: true })).toBe(1);
    expect(b.tabs.query).toHaveBeenCalledWith({ currentWindow: true });
  });

  it("resolves by window_id + active", async () => {
    const b = getBrowser();
    b.tabs.query.mockResolvedValue([{ id: 5, active: true, windowId: 3 }]);
    expect(await helpers.targetTabId({ window_id: 3, active: true })).toBe(5);
    expect(b.tabs.query).toHaveBeenCalledWith({ windowId: 3 });
  });

  it("resolves by url_contains via queryAllTabsSafe", async () => {
    const b = getBrowser();
    b.windows.getAll.mockResolvedValue([{ id: 1, type: "normal" }]);
    b.tabs.query.mockResolvedValue([{ id: 2, title: "Match", url: "https://match.com", active: false }]);
    expect(await helpers.targetTabId({ url_contains: "match" })).toBe(2);
  });

  it("throws when no tabs match", async () => {
    const b = getBrowser();
    b.windows.getAll.mockResolvedValue([{ id: 1, type: "normal" }]);
    b.tabs.query.mockResolvedValue([]);
    await expect(helpers.targetTabId({})).rejects.toThrow();
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// getFrameList
// ═══════════════════════════════════════════════════════════════════════════

describe("getFrameList", () => {
  it("returns sorted frames", async () => {
    const b = getBrowser();
    b.webNavigation.getAllFrames.mockResolvedValue([
      { frameId: 5, parentFrameId: 0, url: "https://iframe.com" },
      { frameId: 0, parentFrameId: -1, url: "https://main.com" },
      { frameId: 3, parentFrameId: 0, url: "https://iframe2.com" },
    ]);
    const frames = await helpers.getFrameList(1);
    expect(b.webNavigation.getAllFrames).toHaveBeenCalledWith({ tabId: 1 });
    // main frame first (parentFrameId === -1)
    expect(frames[0].frameId).toBe(0);
    // then by frameId asc
    expect(frames[1].frameId).toBe(3);
    expect(frames[2].frameId).toBe(5);
  });

  it("handles single frame (main only)", async () => {
    const b = getBrowser();
    b.webNavigation.getAllFrames.mockResolvedValue([
      { frameId: 0, parentFrameId: -1, url: "https://solo.com" },
    ]);
    const frames = await helpers.getFrameList(1);
    expect(frames).toHaveLength(1);
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// execInFrame
// ═══════════════════════════════════════════════════════════════════════════

describe("execInFrame", () => {
  it("executes script in frame and returns result", async () => {
    const b = getBrowser();
    b.tabs.executeScript.mockResolvedValue([42]);
    const result = await helpers.execInFrame(1, 0, "return 42");
    expect(b.tabs.executeScript).toHaveBeenCalledWith(1, { code: "return 42", frameId: 0 });
    expect(result).toBe(42);
  });

  it("returns null on script error", async () => {
    const b = getBrowser();
    b.tabs.executeScript.mockRejectedValue(new Error("script failed"));
    const result = await helpers.execInFrame(1, 0, "bad code");
    expect(result).toBeNull();
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// runPageScript (simple, no frame iteration)
// ═══════════════════════════════════════════════════════════════════════════

describe("runPageScript", () => {
  it("runs script on resolved tab", async () => {
    const b = getBrowser();
    // targetTabId resolves by tab_id
    b.tabs.executeScript.mockResolvedValue([{ title: "Hello" }]);
    const result = await helpers.runPageScript({ tab_id: 1 }, "return document.title");
    expect(b.tabs.executeScript).toHaveBeenCalledWith(1, expect.objectContaining({
      code: "return document.title",
    }));
    expect(result).toEqual({ title: "Hello" });
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// runPageScriptAllFrames
// ═══════════════════════════════════════════════════════════════════════════

describe("runPageScriptAllFrames", () => {
  function mockFrameTest(b) {
    // targetTabId({ active: true }) → tabs.query({ currentWindow: true })
    b.tabs.query.mockResolvedValue([{ id: 1, active: true, url: "https://example.com" }]);
  }

  it("runs in specific frame_index", async () => {
    const b = getBrowser();
    mockFrameTest(b);
    b.webNavigation.getAllFrames.mockResolvedValue([
      { frameId: 0, parentFrameId: -1, url: "https://main.com" },
      { frameId: 7, parentFrameId: 0, url: "https://iframe.com" },
    ]);
    b.tabs.executeScript.mockResolvedValue(["iframe result"]);
    const result = await helpers.runPageScriptAllFrames(
      { active: true, frame_index: 1 },
      "return 'hi'",
    );
    expect(b.tabs.executeScript).toHaveBeenCalledWith(1, { code: "return 'hi'", frameId: 7 });
    expect(result).toBe("iframe result");
  });

  it("returns null when frame_index out of range", async () => {
    const b = getBrowser();
    mockFrameTest(b);
    b.webNavigation.getAllFrames.mockResolvedValue([
      { frameId: 0, parentFrameId: -1, url: "https://main.com" },
    ]);
    const result = await helpers.runPageScriptAllFrames(
      { active: true, frame_index: 5 },
      "return 'x'",
    );
    expect(result).toBeNull();
  });

  it("iterates frames and returns first meaningful result", async () => {
    const b = getBrowser();
    mockFrameTest(b);
    b.webNavigation.getAllFrames.mockResolvedValue([
      { frameId: 0, parentFrameId: -1, url: "https://main.com" },
      { frameId: 3, parentFrameId: 0, url: "https://iframe.com" },
    ]);
    b.tabs.executeScript
      .mockResolvedValueOnce([null])       // frame 0 returns null → skip
      .mockResolvedValueOnce(["found"]);   // frame 3 returns "found"
    const result = await helpers.runPageScriptAllFrames(
      { active: true },
      "return 'x'",
    );
    expect(result).toBe("found");
    expect(b.tabs.executeScript).toHaveBeenCalledTimes(2);
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// runPageScriptText (merge text from all frames)
// ═══════════════════════════════════════════════════════════════════════════

describe("runPageScriptText", () => {
  function mockTextTest(b) {
    b.tabs.query.mockResolvedValue([{ id: 1, active: true, url: "https://example.com" }]);
  }

  it("merges text from all frames", async () => {
    const b = getBrowser();
    mockTextTest(b);
    b.webNavigation.getAllFrames.mockResolvedValue([
      { frameId: 0, parentFrameId: -1, url: "https://main.com" },
      { frameId: 8, parentFrameId: 0, url: "https://iframe.com" },
    ]);
    b.tabs.executeScript
      .mockResolvedValueOnce([{ text: "Main content" }])
      .mockResolvedValueOnce([{ text: "Frame content" }]);
    const result = await helpers.runPageScriptText({ active: true }, "return {text: document.body.innerText}");
    expect(result.text).toContain("Main content");
    expect(result.text).toContain("Frame content");
    expect(result.text).toContain("[frame]");
  });

  it("handles single frame (text)", async () => {
    const b = getBrowser();
    mockTextTest(b);
    b.webNavigation.getAllFrames.mockResolvedValue([
      { frameId: 0, parentFrameId: -1, url: "https://solo.com" },
    ]);
    b.tabs.executeScript.mockResolvedValue([{ text: "Just text" }]);
    const result = await helpers.runPageScriptText({ active: true }, "return {text: 'ok'}");
    expect(result.text).toBe("Just text");
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// runPageScriptSnapshot (merge interactive elements from all frames)
// ═══════════════════════════════════════════════════════════════════════════

describe("runPageScriptSnapshot", () => {
  it("merges elements from all frames", async () => {
    const b = getBrowser();
    b.tabs.query.mockResolvedValue([{ id: 1, active: true, url: "https://example.com" }]);
    b.webNavigation.getAllFrames.mockResolvedValue([
      { frameId: 0, parentFrameId: -1, url: "https://main.com" },
    ]);
    b.tabs.executeScript.mockResolvedValue([{
      elements: [{ tag: "button", text: "OK" }],
      frameElements: [],
    }]);
    const result = await helpers.runPageScriptSnapshot({ active: true }, "return {}");
    expect(result.elements).toHaveLength(1);
    expect(result.elements[0].tag).toBe("button");
    expect(result.elements[0].frameIndex).toBe(0);
    expect(result.elements[0].ref).toMatch(/^f0:e/);
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// restoreSessionOfType
// ═══════════════════════════════════════════════════════════════════════════

describe("restoreSessionOfType", () => {
  it("throws on invalid kind", async () => {
    await expect(helpers.restoreSessionOfType("page", "s1")).rejects.toThrow("invalid session kind");
  });

  it("restores window session by id", async () => {
    const b = getBrowser();
    b.sessions.getRecentlyClosed.mockResolvedValue([
      { window: { sessionId: "ws1" } },
    ]);
    b.sessions.restore.mockResolvedValue({ window: { id: 99 } });
    const result = await helpers.restoreSessionOfType("window", "ws1");
    expect(b.sessions.getRecentlyClosed).toHaveBeenCalledWith({ maxResults: 100 });
    expect(b.sessions.restore).toHaveBeenCalledWith("ws1");
    expect(result.window.id).toBe(99);
  });

  it("restores tab session by id", async () => {
    const b = getBrowser();
    b.sessions.getRecentlyClosed.mockResolvedValue([
      { tab: { sessionId: "ts1" } },
    ]);
    b.sessions.restore.mockResolvedValue({ tab: { id: 7 } });
    const result = await helpers.restoreSessionOfType("tab", "ts1");
    expect(b.sessions.restore).toHaveBeenCalledWith("ts1");
    expect(result.tab.id).toBe(7);
  });

  it("throws when session_id not found in recently closed", async () => {
    const b = getBrowser();
    b.sessions.getRecentlyClosed.mockResolvedValue([
      { tab: { sessionId: "other" } },
    ]);
    await expect(
      helpers.restoreSessionOfType("tab", "nope")
    ).rejects.toThrow("session_id is not a recently closed tab");
  });

  it("finds first recently closed when no id given", async () => {
    const b = getBrowser();
    b.sessions.getRecentlyClosed.mockResolvedValue([
      { window: { sessionId: "auto1" } },
    ]);
    b.sessions.restore.mockResolvedValue({ window: { id: 1 } });
    await helpers.restoreSessionOfType("window");
    expect(b.sessions.restore).toHaveBeenCalledWith("auto1");
  });

  it("throws when nothing recently closed (no id)", async () => {
    const b = getBrowser();
    b.sessions.getRecentlyClosed.mockResolvedValue([]);
    await expect(
      helpers.restoreSessionOfType("tab")
    ).rejects.toThrow("no recently closed tab found");
  });
});
