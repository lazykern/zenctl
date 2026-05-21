/**
 * Tests: tabs handlers (background.js)
 *
 * Covers: tabs_list, tabs_find, tabs_open, tabs_close, tabs_activate,
 * tabs_reload, tabs_duplicate, tabs_discard, tabs_set_muted,
 * tabs_set_pinned, tabs_screenshot, tabs_zoom, tabs_reader,
 * tabs_go_back, tabs_go_forward, tabs_move, tab_group, tab_ungroup.
 */
import { describe, it, expect, beforeAll } from "vitest";
import { getBrowser } from "../setup.js";

let handlers = {};

beforeAll(async () => {
  const mod = await import("../../src/background.js");
  handlers = mod.handlers;
});

// ── Fixtures ────────────────────────────────────────────────────────────

function makeTab(id = 1, overrides = {}) {
  return {
    id,
    windowId: 1,
    url: "https://example.com",
    title: "Example",
    pinned: false,
    mutedInfo: { muted: false },
    incognito: false,
    index: id - 1,
    active: id === 1,
    ...overrides,
  };
}

/**
 * Wire up queryAllTabsSafe path: mock windows.getAll + per-window tabs.query.
 * `windowTabs` maps windowId → visible tabs array.
 * `hiddenTabs` maps windowId → hidden tabs array (defaults to empty).
 */
function mockQueryAllTabs(browser, windowTabs = {}, hiddenTabs = {}) {
  const winIds = Object.keys(windowTabs).map(Number);
  browser.windows.getAll.mockResolvedValue(
    winIds.map((id) => ({ id, type: "normal", focused: id === 1 })),
  );
  browser.tabs.query.mockImplementation(async (q) => {
    if (q.hidden) return hiddenTabs[q.windowId] || [];
    if (q.currentWindow) {
      // simulate: return tabs from focused window
      return windowTabs[1] || [];
    }
    if (q.windowId != null) return windowTabs[q.windowId] || [];
    return [];
  });
}

/**
 * Wire up targetTabId + { active: true } path.
 * Resolves via `tabs.query({ currentWindow: true })` for active target,
 * or `windows.getAll({ populate: true, ... })` for fallback.
 */
function mockActiveTab(browser, tab = makeTab(1)) {
  // The { active: true } path hits `tabs.query({ currentWindow: true })`
  browser.tabs.query.mockResolvedValue([tab]);
  // Fallback path (when no selectors) hits windows.getAll with populate
  browser.windows.getAll.mockResolvedValue([
    { id: 1, type: "normal", tabs: [tab] },
  ]);
}

// ── tabs_list ────────────────────────────────────────────────────────────

describe("tabs_list", () => {
  it("returns all tabs when no args", async () => {
    const browser = getBrowser();
    mockQueryAllTabs(browser, { 1: [makeTab(1), makeTab(2)] });

    const result = await handlers.tabs_list({});
    expect(result).toHaveLength(2);
    expect(result[0].id).toBe(1);
    expect(result[1].id).toBe(2);
  });

  it("queries by window_id (skips queryAllTabsSafe)", async () => {
    const browser = getBrowser();
    browser.tabs.query.mockResolvedValue([makeTab(1, { windowId: 5 })]);

    const result = await handlers.tabs_list({ window_id: 5 });
    expect(result).toHaveLength(1);
    expect(result[0].windowId).toBe(5);
    expect(browser.tabs.query).toHaveBeenCalledWith({ windowId: 5 });
  });

  it("queries current window", async () => {
    const browser = getBrowser();
    browser.tabs.query.mockResolvedValue([makeTab(1)]);

    await handlers.tabs_list({ current_window: true });
    expect(browser.tabs.query).toHaveBeenCalledWith({ currentWindow: true });
  });

  it("returns empty when no windows have tabs", async () => {
    mockQueryAllTabs(getBrowser(), { 1: [] });
    const result = await handlers.tabs_list({});
    expect(result).toEqual([]);
  });

  it("includes hidden tabs from queryAllTabsSafe", async () => {
    const browser = getBrowser();
    mockQueryAllTabs(
      browser,
      { 1: [makeTab(1, { title: "visible" })] },
      { 1: [makeTab(2, { title: "hidden" })] },
    );

    const result = await handlers.tabs_list({});
    expect(result).toHaveLength(2);
    expect(result.map((t) => t.title).sort()).toEqual(
      expect.arrayContaining(["visible", "hidden"]),
    );
  });
});

// ── tabs_find ────────────────────────────────────────────────────────────

describe("tabs_find", () => {
  it("finds tab by id (skips queryAllTabsSafe)", async () => {
    const browser = getBrowser();
    browser.tabs.get.mockResolvedValue(makeTab(7));

    const result = await handlers.tabs_find({ tab_id: 7 });
    expect(result).toHaveLength(1);
    expect(result[0].id).toBe(7);
  });

  it("returns empty when tab_id not found", async () => {
    getBrowser().tabs.get.mockRejectedValue(new Error("not found"));
    const result = await handlers.tabs_find({ tab_id: 999 });
    expect(result).toEqual([]);
  });

  it("filters by url_contains via queryAllTabsSafe", async () => {
    const browser = getBrowser();
    mockQueryAllTabs(browser, {
      1: [
        makeTab(1, { url: "https://github.com" }),
        makeTab(2, { url: "https://example.com" }),
      ],
    });

    const result = await handlers.tabs_find({ url_contains: "github" });
    expect(result).toHaveLength(1);
    expect(result[0].url).toBe("https://github.com");
  });

  it("filters by title_contains via queryAllTabsSafe", async () => {
    const browser = getBrowser();
    mockQueryAllTabs(browser, {
      1: [
        makeTab(1, { title: "Zen Browser" }),
        makeTab(2, { title: "Firefox" }),
      ],
    });

    const result = await handlers.tabs_find({ title_contains: "Zen" });
    expect(result).toHaveLength(1);
    expect(result[0].title).toBe("Zen Browser");
  });

  it("returns empty when no match", async () => {
    const browser = getBrowser();
    mockQueryAllTabs(browser, { 1: [makeTab(1, { title: "Alpha" })] });

    const result = await handlers.tabs_find({ title_contains: "Omega" });
    expect(result).toEqual([]);
  });

  it("combines url + title filters", async () => {
    const browser = getBrowser();
    mockQueryAllTabs(browser, {
      1: [
        makeTab(1, { url: "https://zen.org", title: "Welcome" }),
        makeTab(2, { url: "https://other.io", title: "Zen" }),
      ],
    });
    // url must contain "zen" AND title must contain "Welcome"
    const result = await handlers.tabs_find({
      url_contains: "zen",
      title_contains: "Welcome",
    });
    expect(result).toHaveLength(1);
    expect(result[0].id).toBe(1);
  });

  it("filters by window_id", async () => {
    const browser = getBrowser();
    browser.tabs.query.mockResolvedValue([
      makeTab(1, { windowId: 3 }),
      makeTab(2, { windowId: 5 }),
    ]);
    // When explicit window_id is given, targetTabId queries with windowId
    // directly (not via queryAllTabsSafe), but tabs_find calls
    // queryAllTabsSafe when tab_id is absent, so let's test
    // the filterTabs path
    mockQueryAllTabs(browser, {
      3: [makeTab(1, { windowId: 3 })],
    });
    const result = await handlers.tabs_find({ window_id: 3 });
    expect(result).toHaveLength(1);
    expect(result[0].windowId).toBe(3);
  });
});

// ── tabs_open ────────────────────────────────────────────────────────────

describe("tabs_open", () => {
  it("throws when url missing", async () => {
    await expect(handlers.tabs_open({})).rejects.toThrow("url required");
  });

  it("opens tab with explicit window_id", async () => {
    const browser = getBrowser();
    browser.tabs.create.mockResolvedValue(makeTab(42));

    const result = await handlers.tabs_open({
      url: "https://zen.org",
      window_id: 2,
    });
    expect(browser.tabs.create).toHaveBeenCalledWith({
      url: "https://zen.org",
      active: true,
      windowId: 2,
    });
    expect(result.id).toBe(42);
  });

  it("opens tab without window_id via window fallback", async () => {
    const browser = getBrowser();
    browser.windows.getAll.mockResolvedValue([
      { id: 1, type: "normal" },
      { id: 2, type: "normal" },
    ]);
    browser.tabs.create
      .mockRejectedValueOnce(new Error("window 1 rejects"))
      .mockResolvedValueOnce(makeTab(7, { windowId: 2 }));

    const result = await handlers.tabs_open({ url: "https://example.com" });
    expect(result.id).toBe(7);
    // second attempt succeeded
    expect(browser.tabs.create).toHaveBeenCalledTimes(2);
  });

  it("opens tab as background (active=false)", async () => {
    const browser = getBrowser();
    browser.windows.getAll.mockResolvedValue([
      { id: 1, type: "normal" },
    ]);
    browser.tabs.create.mockResolvedValue(makeTab(1));

    await handlers.tabs_open({ url: "https://example.com", active: false });
    expect(browser.tabs.create).toHaveBeenCalledWith(
      expect.objectContaining({ active: false }),
    );
  });

  it("throws last error when all windows reject", async () => {
    const browser = getBrowser();
    browser.windows.getAll.mockResolvedValue([
      { id: 1, type: "normal" },
    ]);
    browser.tabs.create.mockRejectedValue(new Error("all dead"));

    await expect(
      handlers.tabs_open({ url: "https://example.com" }),
    ).rejects.toThrow("all dead");
  });
});

// ── tabs_close ───────────────────────────────────────────────────────────

describe("tabs_close", () => {
  it("throws when no id provided", async () => {
    await expect(handlers.tabs_close({})).rejects.toThrow(
      "tab_id or tab_ids required",
    );
  });

  it("closes single tab via tab_id", async () => {
    const browser = getBrowser();
    const result = await handlers.tabs_close({ tab_id: 5 });
    expect(browser.tabs.remove).toHaveBeenCalledWith([5]);
    expect(result.closed).toBe(1);
  });

  it("closes multiple tabs via tab_ids", async () => {
    const browser = getBrowser();
    await handlers.tabs_close({ tab_ids: [1, 2, 3] });
    expect(browser.tabs.remove).toHaveBeenCalledWith([1, 2, 3]);
  });
});

// ── tabs_activate ────────────────────────────────────────────────────────

describe("tabs_activate", () => {
  it("throws when tab_id missing", async () => {
    await expect(handlers.tabs_activate({})).rejects.toThrow(
      "tab_id required",
    );
  });

  it("activates specified tab", async () => {
    const browser = getBrowser();
    browser.tabs.update.mockResolvedValue(makeTab(3));
    await handlers.tabs_activate({ tab_id: 3 });
    expect(browser.tabs.update).toHaveBeenCalledWith(3, { active: true });
  });
});

// ── tabs_reload ──────────────────────────────────────────────────────────

describe("tabs_reload", () => {
  it("reloads active tab with bypass_cache", async () => {
    const browser = getBrowser();
    mockActiveTab(browser, makeTab(1));
    const result = await handlers.tabs_reload({
      active: true,
      bypass_cache: true,
    });
    expect(browser.tabs.reload).toHaveBeenCalledWith(1, {
      bypassCache: true,
    });
    expect(result.tab_id).toBe(1);
  });

  it("reloads by tab_id", async () => {
    const browser = getBrowser();
    const result = await handlers.tabs_reload({ tab_id: 5 });
    expect(browser.tabs.reload).toHaveBeenCalledWith(5, {
      bypassCache: false,
    });
    expect(result.tab_id).toBe(5);
  });
});

// ── tabs_duplicate ───────────────────────────────────────────────────────

describe("tabs_duplicate", () => {
  it("duplicates active tab", async () => {
    const browser = getBrowser();
    mockActiveTab(browser, makeTab(5));
    browser.tabs.duplicate.mockResolvedValue(makeTab(99));
    await handlers.tabs_duplicate({ active: true });
    expect(browser.tabs.duplicate).toHaveBeenCalledWith(5);
  });

  it("duplicates by tab_id", async () => {
    const browser = getBrowser();
    browser.tabs.duplicate.mockResolvedValue(makeTab(42));
    const result = await handlers.tabs_duplicate({ tab_id: 7 });
    expect(browser.tabs.duplicate).toHaveBeenCalledWith(7);
    expect(result.id).toBe(42);
  });
});

// ── tabs_discard ─────────────────────────────────────────────────────────

describe("tabs_discard", () => {
  it("discards active tab", async () => {
    const browser = getBrowser();
    mockActiveTab(browser, makeTab(3));
    const result = await handlers.tabs_discard({ active: true });
    expect(browser.tabs.discard).toHaveBeenCalledWith(3);
    expect(result.discarded).toBe(true);
    expect(result.tab_id).toBe(3);
  });
});

// ── tabs_set_muted ───────────────────────────────────────────────────────

describe("tabs_set_muted", () => {
  it("mutes active tab", async () => {
    const browser = getBrowser();
    mockActiveTab(browser, makeTab(1));
    browser.tabs.update.mockResolvedValue(
      makeTab(1, { mutedInfo: { muted: true } }),
    );
    const result = await handlers.tabs_set_muted({ active: true, muted: true });
    expect(browser.tabs.update).toHaveBeenCalledWith(1, { muted: true });
    expect(result.muted).toBe(true);
  });

  it("unmutes active tab", async () => {
    const browser = getBrowser();
    mockActiveTab(browser, makeTab(1));
    browser.tabs.update.mockResolvedValue(
      makeTab(1, { mutedInfo: { muted: false } }),
    );
    const result = await handlers.tabs_set_muted({
      active: true,
      muted: false,
    });
    expect(browser.tabs.update).toHaveBeenCalledWith(1, { muted: false });
    expect(result.muted).toBe(false);
  });
});

// ── tabs_set_pinned ──────────────────────────────────────────────────────

describe("tabs_set_pinned", () => {
  it("pins active tab", async () => {
    const browser = getBrowser();
    mockActiveTab(browser, makeTab(1));
    browser.tabs.update.mockResolvedValue(makeTab(1, { pinned: true }));
    const result = await handlers.tabs_set_pinned({
      active: true,
      pinned: true,
    });
    expect(browser.tabs.update).toHaveBeenCalledWith(1, { pinned: true });
    expect(result.pinned).toBe(true);
  });

  it("unpins active tab", async () => {
    const browser = getBrowser();
    mockActiveTab(browser, makeTab(1));
    browser.tabs.update.mockResolvedValue(makeTab(1, { pinned: false }));
    const result = await handlers.tabs_set_pinned({
      active: true,
      pinned: false,
    });
    expect(browser.tabs.update).toHaveBeenCalledWith(1, { pinned: false });
    expect(result.pinned).toBe(false);
  });
});

// ── tabs_move ────────────────────────────────────────────────────────────

describe("tabs_move", () => {
  it("throws when no tab_id or tab_ids", async () => {
    await expect(handlers.tabs_move({ index: 0 })).rejects.toThrow(
      "tab_id or tab_ids required",
    );
  });

  it("throws when no index", async () => {
    await expect(handlers.tabs_move({ tab_id: 1 })).rejects.toThrow(
      "index required",
    );
  });

  it("moves a tab within same window", async () => {
    const browser = getBrowser();
    browser.tabs.move.mockResolvedValue([makeTab(1)]);
    const result = await handlers.tabs_move({ tab_id: 1, index: 3 });
    expect(browser.tabs.move).toHaveBeenCalledWith([1], { index: 3 });
    expect(result).toHaveLength(1);
  });

  it("moves tab to different window", async () => {
    const browser = getBrowser();
    await handlers.tabs_move({ tab_id: 1, index: 0, window_id: 5 });
    expect(browser.tabs.move).toHaveBeenCalledWith([1], {
      index: 0,
      windowId: 5,
    });
  });

  it("moves multiple tabs", async () => {
    const browser = getBrowser();
    await handlers.tabs_move({ tab_ids: [1, 2, 3], index: 0 });
    expect(browser.tabs.move).toHaveBeenCalledWith([1, 2, 3], { index: 0 });
  });
});

// ── tabs_zoom ────────────────────────────────────────────────────────────

describe("tabs_zoom", () => {
  it("returns current zoom", async () => {
    const browser = getBrowser();
    mockActiveTab(browser, makeTab(1));
    browser.tabs.getZoom.mockResolvedValue(1.5);
    const result = await handlers.tabs_zoom({ active: true });
    expect(result.zoom).toBe(1.5);
    expect(result.tab_id).toBe(1);
  });

  it("sets zoom then returns new zoom", async () => {
    const browser = getBrowser();
    mockActiveTab(browser, makeTab(1));
    browser.tabs.getZoom.mockResolvedValue(2.0);
    const result = await handlers.tabs_zoom({ active: true, value: 2.0 });
    expect(browser.tabs.setZoom).toHaveBeenCalledWith(1, 2.0);
    expect(result.zoom).toBe(2.0);
  });
});

// ── tabs_reader ──────────────────────────────────────────────────────────

describe("tabs_reader", () => {
  it("toggles reader mode on active tab", async () => {
    const browser = getBrowser();
    mockActiveTab(browser, makeTab(1));
    const r = await handlers.tabs_reader({ active: true });
    expect(browser.tabs.toggleReaderMode).toHaveBeenCalledWith(1);
    expect(r.toggled).toBe(true);
  });
});

// ── tabs_go_back / tabs_go_forward ───────────────────────────────────────

describe("tabs_go_back", () => {
  it("navigates back", async () => {
    const browser = getBrowser();
    mockActiveTab(browser, makeTab(1));
    const r = await handlers.tabs_go_back({ active: true });
    expect(browser.tabs.goBack).toHaveBeenCalledWith(1);
    expect(r.navigated).toBe("back");
  });
});

describe("tabs_go_forward", () => {
  it("navigates forward", async () => {
    const browser = getBrowser();
    mockActiveTab(browser, makeTab(1));
    const r = await handlers.tabs_go_forward({ active: true });
    expect(browser.tabs.goForward).toHaveBeenCalledWith(1);
    expect(r.navigated).toBe("forward");
  });
});

// ── tab_group ────────────────────────────────────────────────────────────

describe("tab_group", () => {
  it("throws when tab_ids missing", async () => {
    await expect(handlers.tab_group({})).rejects.toThrow("tab_ids required");
  });

  it("groups tabs and returns group_id", async () => {
    const browser = getBrowser();
    browser.tabs.group.mockResolvedValue(7);
    const result = await handlers.tab_group({ tab_ids: [1, 2, 3] });
    expect(browser.tabs.group).toHaveBeenCalledWith({ tabIds: [1, 2, 3] });
    expect(result.group_id).toBe(7);
    expect(result.tab_ids).toEqual([1, 2, 3]);
  });

  it("adds to existing group when group_id provided", async () => {
    const browser = getBrowser();
    browser.tabs.group.mockResolvedValue(5);
    await handlers.tab_group({ tab_ids: [4], group_id: 5 });
    expect(browser.tabs.group).toHaveBeenCalledWith({
      tabIds: [4],
      groupId: 5,
    });
  });

  it("throws if group API unavailable", async () => {
    const browser = getBrowser();
    browser.tabs.group.mockRejectedValue(
      new Error("group API unavailable"),
    );
    await expect(handlers.tab_group({ tab_ids: [1] })).rejects.toThrow(
      "group API unavailable",
    );
  });
});

// ── tab_ungroup ──────────────────────────────────────────────────────────

describe("tab_ungroup", () => {
  it("throws when tab_ids missing", async () => {
    await expect(handlers.tab_ungroup({})).rejects.toThrow(
      "tab_ids required",
    );
  });

  it("throws when tab_ids is empty array", async () => {
    await expect(handlers.tab_ungroup({ tab_ids: [] })).rejects.toThrow(
      "tab_ids required",
    );
  });

  it("ungroups tabs", async () => {
    const browser = getBrowser();
    const result = await handlers.tab_ungroup({ tab_ids: [1, 2] });
    expect(browser.tabs.ungroup).toHaveBeenCalledWith([1, 2]);
    expect(result.ungrouped).toEqual([1, 2]);
  });

  it("throws if ungroup API unavailable", async () => {
    const browser = getBrowser();
    // Override the mock to throw — simulating build without group API
    browser.tabs.ungroup.mockRejectedValue(
      new Error("browser.tabs.ungroup unavailable in this Firefox/Zen build"),
    );
    await expect(handlers.tab_ungroup({ tab_ids: [1] })).rejects.toThrow(
      "browser.tabs.ungroup unavailable",
    );
  });
});
