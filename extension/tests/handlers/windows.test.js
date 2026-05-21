/**
 * Tests: windows handlers (windows_list, windows_focus, windows_close,
 * windows_create, windows_update, window_sync_force).
 */
import { describe, it, expect, beforeAll } from "vitest";
import { getBrowser } from "../setup.js";

let handlers = {};
beforeAll(async () => {
  handlers = (await import("../../src/background.js")).handlers;
});

// ── Fixture ──────────────────────────────────────────────────────────────

function makeWin(id = 1, overrides = {}) {
  return { id, type: "normal", focused: false, incognito: false, state: "normal", ...overrides };
}

// ── windows_list ─────────────────────────────────────────────────────────

describe("windows_list", () => {
  it("returns all windows", async () => {
    const browser = getBrowser();
    browser.windows.getAll.mockResolvedValue([makeWin(1), makeWin(2)]);
    const result = await handlers.windows_list();
    expect(result).toHaveLength(2);
    expect(browser.windows.getAll).toHaveBeenCalledWith({ populate: false });
  });
  it("returns empty when no windows", async () => {
    getBrowser().windows.getAll.mockResolvedValue([]);
    expect(await handlers.windows_list()).toEqual([]);
  });
});

// ── windows_focus ────────────────────────────────────────────────────────

describe("windows_focus", () => {
  it("throws when window_id missing", async () => {
    await expect(handlers.windows_focus({})).rejects.toThrow("window_id required");
  });
  it("focuses window", async () => {
    const browser = getBrowser();
    browser.windows.update.mockResolvedValue(makeWin(3, { focused: true }));
    const result = await handlers.windows_focus({ window_id: 3 });
    expect(browser.windows.update).toHaveBeenCalledWith(3, { focused: true });
    expect(result.focused).toBe(true);
  });
});

// ── windows_close ────────────────────────────────────────────────────────

describe("windows_close", () => {
  it("throws when window_id missing", async () => {
    await expect(handlers.windows_close({})).rejects.toThrow("window_id required");
  });
  it("closes window and reports back", async () => {
    const browser = getBrowser();
    const result = await handlers.windows_close({ window_id: 7 });
    expect(browser.windows.remove).toHaveBeenCalledWith(7);
    expect(result).toEqual({ closed: true, window_id: 7 });
  });
});

// ── windows_create ───────────────────────────────────────────────────────

describe("windows_create", () => {
  it("creates with url only", async () => {
    const browser = getBrowser();
    browser.windows.create.mockResolvedValue(makeWin(99));
    await handlers.windows_create({ url: "https://example.com" });
    expect(browser.windows.create).toHaveBeenCalledWith({ url: "https://example.com" });
  });
  it("strips falsy keys", async () => {
    const browser = getBrowser();
    browser.windows.create.mockResolvedValue(makeWin(1));
    await handlers.windows_create({});
    expect(browser.windows.create).toHaveBeenCalledWith({});
  });
  it("passes incognito + state + type", async () => {
    const browser = getBrowser();
    browser.windows.create.mockResolvedValue(makeWin(2));
    await handlers.windows_create({ incognito: true, state: "maximized", type: "popup" });
    expect(browser.windows.create).toHaveBeenCalledWith({
      incognito: true, state: "maximized", type: "popup",
    });
  });
});

// ── windows_update ───────────────────────────────────────────────────────

describe("windows_update", () => {
  it("throws when window_id missing", async () => {
    await expect(handlers.windows_update({ state: "minimized" })).rejects.toThrow("window_id required");
  });
  it("throws when nothing to update", async () => {
    await expect(handlers.windows_update({ window_id: 1 })).rejects.toThrow("nothing to update");
  });
  it("updates state", async () => {
    const browser = getBrowser();
    browser.windows.update.mockResolvedValue(makeWin(1, { state: "minimized" }));
    const result = await handlers.windows_update({ window_id: 1, state: "minimized" });
    expect(browser.windows.update).toHaveBeenCalledWith(1, { state: "minimized" });
    expect(result.state).toBe("minimized");
  });
  it("updates focused", async () => {
    const browser = getBrowser();
    await handlers.windows_update({ window_id: 2, focused: true });
    expect(browser.windows.update).toHaveBeenCalledWith(2, { focused: true });
  });
});

// ── window_sync_force ────────────────────────────────────────────────────

describe("window_sync_force", () => {
  it("triggers window sync via zenChrome", async () => {
    const b = getBrowser();
    b.zenChrome.windowSyncForce.mockResolvedValue(undefined);
    await expect(handlers.window_sync_force()).resolves.toBeUndefined();
    expect(b.zenChrome.windowSyncForce).toHaveBeenCalled();
  });
});
