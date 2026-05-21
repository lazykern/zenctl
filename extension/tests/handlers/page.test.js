/**
 * Tests: page + media handlers.
 *
 * page_frames, page_info, page_text, page_source, page_snapshot,
 * page_click, page_type, page_key, page_wait, page_eval,
 * media_status, media_play, media_pause, media_toggle, media_next,
 * media_previous.
 */
import { describe, it, expect, beforeAll } from "vitest";
import { getBrowser } from "../setup.js";

let handlers = {};
beforeAll(async () => {
  handlers = (await import("../../src/background.js")).handlers;
});

// ── Resolve targetTabId for { active: true } path ────────────────────────
function mockActiveTab(b, tab = { id: 1, url: "https://example.com", active: true }) {
  b.tabs.query.mockResolvedValue([tab]);
  b.windows.getAll.mockResolvedValue([{ id: 1, type: "normal", tabs: [tab] }]);
}

// ── Wire up getFrameList / execInFrame path ──────────────────────────────
function mockFrames(b, tabId = 1, frames = [{ frameId: 0, parentFrameId: -1, url: "https://x.com" }]) {
  b.webNavigation.getAllFrames.mockResolvedValue(frames);
}

// ── page_frames ──────────────────────────────────────────────────────────

describe("page_frames", () => {
  it("returns frame list for active tab", async () => {
    const b = getBrowser();
    mockActiveTab(b);
    mockFrames(b, 1, [
      { frameId: 0, parentFrameId: -1, url: "https://main.com" },
      { frameId: 1, parentFrameId: 0, url: "https://iframe.com" },
    ]);
    const result = await handlers.page_frames({ active: true });
    expect(b.webNavigation.getAllFrames).toHaveBeenCalledWith({ tabId: 1 });
    expect(result.frames).toHaveLength(2);
    expect(result.frames[0].frameId).toBe(0);
    expect(result.frames[0].parentFrameId).toBe(-1);
  });
});

// ── page_info / page_text / page_source ──────────────────────────────────

describe("page_info", () => {
  it("runs info script on active tab", async () => {
    const b = getBrowser();
    mockActiveTab(b);
    b.tabs.executeScript.mockResolvedValue([{ title: "Page Title", url: "https://x.com" }]);
    const result = await handlers.page_info({ active: true });
    const call = b.tabs.executeScript.mock.calls[0];
    expect(call[0]).toBe(1);
    expect(call[1].code).toContain("actions");
    expect(result.title).toBe("Page Title");
  });
});

describe("page_text", () => {
  it("extracts text from active tab", async () => {
    const b = getBrowser();
    mockActiveTab(b);
    mockFrames(b, 1);
    b.tabs.executeScript.mockResolvedValue([{ text: "Hello world" }]);
    const result = await handlers.page_text({ active: true });
    expect(result.text).toBe("Hello world");
  });
});

describe("page_source", () => {
  it("returns source for active tab", async () => {
    const b = getBrowser();
    mockActiveTab(b);
    b.tabs.executeScript.mockResolvedValue(["<!DOCTYPE html><html>...</html>"]);
    const result = await handlers.page_source({ active: true });
    expect(result).toBe("<!DOCTYPE html><html>...</html>");
  });
});

// ── page_snapshot ────────────────────────────────────────────────────────

describe("page_snapshot", () => {
  it("returns snapshot with default limit", async () => {
    const b = getBrowser();
    mockActiveTab(b);
    mockFrames(b, 1);
    b.tabs.executeScript.mockResolvedValue([{ elements: [{ tag: "button", text: "OK" }], frameElements: [] }]);
    const result = await handlers.page_snapshot({ active: true });
    const callS = b.tabs.executeScript.mock.calls[0];
    expect(callS[0]).toBe(1);
    expect(callS[1].code).toContain("50");
    expect(result.elements).toHaveLength(1);
  });

  it("respects custom limit", async () => {
    const b = getBrowser();
    mockActiveTab(b);
    mockFrames(b, 1);
    b.tabs.executeScript.mockResolvedValue([{ elements: [], frameElements: [] }]);
    await handlers.page_snapshot({ active: true, limit: 100 });
    const callL = b.tabs.executeScript.mock.calls[0];
    expect(callL[0]).toBe(1);
    expect(callL[1].code).toContain('"limit":100');
  });
});

// ── page_click / page_type / page_key / page_wait / page_eval ───────────

describe("page_click", () => {
  it("clicks element by selector", async () => {
    const b = getBrowser();
    mockActiveTab(b);
    mockFrames(b, 1);
    b.tabs.executeScript.mockResolvedValue([{ clicked: true, text: "Button" }]);
    const result = await handlers.page_click({ active: true, selector: "#btn" });
    expect(result.clicked).toBe(true);
    expect(result.text).toBe("Button");
  });

  it("handles ref-based targeting (normalizePageRef)", async () => {
    const b = getBrowser();
    mockActiveTab(b);
    mockFrames(b, 1);
    b.tabs.executeScript.mockResolvedValue([{ clicked: true }]);
    const result = await handlers.page_click({ active: true, selector: "a", ref: "f0:e5" });
    expect(result.clicked).toBe(true);
  });
});

describe("page_type", () => {
  it("types text into element", async () => {
    const b = getBrowser();
    mockActiveTab(b);
    mockFrames(b, 1);
    b.tabs.executeScript.mockResolvedValue([{ typed: true, value: "hello" }]);
    const result = await handlers.page_type({ active: true, selector: "input", text: "hello" });
    expect(result.typed).toBe(true);
  });

  it("types with submit", async () => {
    const b = getBrowser();
    mockActiveTab(b);
    mockFrames(b, 1);
    b.tabs.executeScript.mockResolvedValue([{ typed: true, submit: true }]);
    const result = await handlers.page_type({
      active: true, selector: "input.q", text: "query", submit: true,
    });
    expect(result.submit).toBe(true);
  });
});

describe("page_key", () => {
  it("sends key event", async () => {
    const b = getBrowser();
    mockActiveTab(b);
    mockFrames(b, 1);
    b.tabs.executeScript.mockResolvedValue([{ key: "Enter" }]);
    const result = await handlers.page_key({ active: true, key: "Enter" });
    expect(result.key).toBe("Enter");
  });
});

describe("page_wait", () => {
  it("waits for selector with custom timeout", async () => {
    const b = getBrowser();
    mockActiveTab(b);
    mockFrames(b, 1);
    b.tabs.executeScript.mockResolvedValue([{ elapsed: 500, found: true }]);
    const result = await handlers.page_wait({
      active: true, selector: ".loaded", timeout: 3000,
    });
    expect(result.found).toBe(true);
    expect(result.elapsed).toBe(500);
  });
});

describe("page_eval", () => {
  it("throws when code missing", async () => {
    await expect(handlers.page_eval({ active: true })).rejects.toThrow("code required");
  });

  it("evaluates arbitrary JS", async () => {
    const b = getBrowser();
    mockActiveTab(b);
    mockFrames(b, 1);
    b.tabs.executeScript.mockResolvedValue([42]);
    const result = await handlers.page_eval({ active: true, code: "return 42" });
    expect(result).toBe(42);
  });
});

// ── media_status / play / pause / toggle / next / prev ──────────────────

describe("media_status", () => {
  it("returns media status", async () => {
    const b = getBrowser();
    mockActiveTab(b);
    b.tabs.executeScript.mockResolvedValue([{ playing: true, title: "Song" }]);
    const result = await handlers.media_status({ active: true });
    expect(result.playing).toBe(true);
  });
});

describe("media_play", () => {
  it("plays media", async () => {
    const b = getBrowser();
    mockActiveTab(b);
    b.tabs.executeScript.mockResolvedValue([{ success: true }]);
    const result = await handlers.media_play({ active: true });
    expect(result.success).toBe(true);
  });
});

describe("media_pause", () => {
  it("pauses media", async () => {
    const b = getBrowser();
    mockActiveTab(b);
    b.tabs.executeScript.mockResolvedValue([{ success: true }]);
    const result = await handlers.media_pause({ active: true });
    expect(result.success).toBe(true);
  });
});

describe("media_toggle", () => {
  it("toggles media", async () => {
    const b = getBrowser();
    mockActiveTab(b);
    b.tabs.executeScript.mockResolvedValue([{ success: true }]);
    const result = await handlers.media_toggle({ active: true });
    expect(result.success).toBe(true);
  });
});

describe("media_next", () => {
  it("skips to next media", async () => {
    const b = getBrowser();
    mockActiveTab(b);
    b.tabs.executeScript.mockResolvedValue([{ success: true }]);
    const result = await handlers.media_next({ active: true });
    expect(result.success).toBe(true);
  });
});

describe("media_previous", () => {
  it("skips to previous media", async () => {
    const b = getBrowser();
    mockActiveTab(b);
    b.tabs.executeScript.mockResolvedValue([{ success: true }]);
    const result = await handlers.media_previous({ active: true });
    expect(result.success).toBe(true);
  });
});
