/**
 * Tests: history, containers, cookies, sessions, downloads handlers.
 */
import { describe, it, expect, beforeAll } from "vitest";
import { getBrowser } from "../setup.js";

let handlers = {};
beforeAll(async () => {
  handlers = (await import("../../src/background.js")).handlers;
});

// ═══════════════════════════════════════════════════════════════════════════
// HISTORY
// ═══════════════════════════════════════════════════════════════════════════

describe("history_search", () => {
  it("searches with default params", async () => {
    const browser = getBrowser();
    browser.history.search.mockResolvedValue([{ id: "h1", url: "https://a.com" }]);
    const result = await handlers.history_search({ query: "test" });
    expect(browser.history.search).toHaveBeenCalledWith({ text: "test", maxResults: 50 });
    expect(result).toHaveLength(1);
  });
  it("passes max_results and start_time", async () => {
    const browser = getBrowser();
    await handlers.history_search({ query: "x", max_results: 10, start_time: 1000 });
    expect(browser.history.search).toHaveBeenCalledWith({ text: "x", maxResults: 10, startTime: 1000 });
  });
});

describe("history_delete", () => {
  it("throws when url missing", async () => {
    await expect(handlers.history_delete({})).rejects.toThrow("url required");
  });
  it("deletes url and reports back", async () => {
    const browser = getBrowser();
    const result = await handlers.history_delete({ url: "https://bad.com" });
    expect(browser.history.deleteUrl).toHaveBeenCalledWith({ url: "https://bad.com" });
    expect(result).toEqual({ deleted: "https://bad.com" });
  });
});

describe("history_add", () => {
  it("throws when url missing", async () => {
    await expect(handlers.history_add({ title: "X" })).rejects.toThrow("url required");
  });
  it("adds with title", async () => {
    const browser = getBrowser();
    await handlers.history_add({ url: "https://new.com", title: "New Page" });
    expect(browser.history.addUrl).toHaveBeenCalledWith({ url: "https://new.com", title: "New Page" });
  });
  it("adds without title", async () => {
    const browser = getBrowser();
    await handlers.history_add({ url: "https://notitle.com" });
    expect(browser.history.addUrl).toHaveBeenCalledWith({ url: "https://notitle.com" });
  });
});

describe("history_get_visits", () => {
  it("returns visit details", async () => {
    const browser = getBrowser();
    browser.history.getVisits.mockResolvedValue([{ visitId: "v1", visitTime: 1 }]);
    const result = await handlers.history_get_visits({ url: "https://a.com" });
    expect(browser.history.getVisits).toHaveBeenCalledWith({ url: "https://a.com" });
    expect(result).toHaveLength(1);
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// CONTAINERS
// ═══════════════════════════════════════════════════════════════════════════

describe("containers_list", () => {
  it("returns containers", async () => {
    const browser = getBrowser();
    browser.contextualIdentities.query.mockResolvedValue([
      { cookieStoreId: "firefox-container-1", name: "Personal" },
    ]);
    const result = await handlers.containers_list();
    expect(result).toHaveLength(1);
    expect(result[0].name).toBe("Personal");
  });
});

describe("containers_create", () => {
  it("creates with defaults", async () => {
    const browser = getBrowser();
    browser.contextualIdentities.create.mockResolvedValue({ cookieStoreId: "c1" });
    const result = await handlers.containers_create({ name: "Work" });
    expect(browser.contextualIdentities.create).toHaveBeenCalledWith({
      name: "Work", color: "blue", icon: "circle",
    });
    expect(result.cookieStoreId).toBe("c1");
  });
  it("creates with custom color and icon", async () => {
    const browser = getBrowser();
    await handlers.containers_create({ name: "X", color: "red", icon: "fingerprint" });
    expect(browser.contextualIdentities.create).toHaveBeenCalledWith({
      name: "X", color: "red", icon: "fingerprint",
    });
  });
});

describe("containers_update", () => {
  it("updates container with deltas", async () => {
    const browser = getBrowser();
    browser.contextualIdentities.update.mockResolvedValue({ cookieStoreId: "c1", name: "Updated" });
    await handlers.containers_update({ cookie_store_id: "c1", name: "Updated", color: "green" });
    expect(browser.contextualIdentities.update).toHaveBeenCalledWith("c1", { name: "Updated", color: "green" });
  });
  it("updates with icon only", async () => {
    const browser = getBrowser();
    await handlers.containers_update({ cookie_store_id: "c2", icon: "gift" });
    expect(browser.contextualIdentities.update).toHaveBeenCalledWith("c2", { icon: "gift" });
  });
});

describe("containers_remove", () => {
  it("removes container", async () => {
    const browser = getBrowser();
    browser.contextualIdentities.query.mockResolvedValue([
      { cookieStoreId: "c9", name: "Test" },
    ]);
    await handlers.containers_remove({ cookie_store_id: "c9" });
    expect(browser.contextualIdentities.remove).toHaveBeenCalledWith("c9");
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// COOKIES
// ═══════════════════════════════════════════════════════════════════════════

describe("cookies_get", () => {
  it("returns cookie", async () => {
    const browser = getBrowser();
    browser.cookies.get.mockResolvedValue({ name: "session", value: "abc" });
    const result = await handlers.cookies_get({ url: "https://x.com", name: "session" });
    expect(browser.cookies.get).toHaveBeenCalledWith({ url: "https://x.com", name: "session" });
    expect(result.name).toBe("session");
  });
});

describe("cookies_set", () => {
  it("sets cookie with all fields", async () => {
    const browser = getBrowser();
    browser.cookies.set.mockResolvedValue({ name: "t", value: "1" });
    await handlers.cookies_set({
      url: "https://a.com", name: "t", value: "1", domain: ".a.com",
      path: "/", secure: true, http_only: true, expiry: 9999,
    });
    expect(browser.cookies.set).toHaveBeenCalledWith({
      url: "https://a.com", name: "t", value: "1", domain: ".a.com",
      path: "/", secure: true, httpOnly: true, expirationDate: 9999,
    });
  });
  it("sets cookie with minimal fields", async () => {
    const browser = getBrowser();
    await handlers.cookies_set({ url: "https://b.com", name: "k", value: "v" });
    expect(browser.cookies.set).toHaveBeenCalledWith({ url: "https://b.com", name: "k", value: "v" });
  });
});

describe("cookies_remove", () => {
  it("removes cookie", async () => {
    const browser = getBrowser();
    browser.cookies.remove.mockResolvedValue({ name: "old", url: "https://x.com" });
    const result = await handlers.cookies_remove({ url: "https://x.com", name: "old" });
    expect(browser.cookies.remove).toHaveBeenCalledWith({ url: "https://x.com", name: "old" });
    expect(result.name).toBe("old");
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// SESSIONS
// ═══════════════════════════════════════════════════════════════════════════

describe("sessions_closed", () => {
  it("returns recently closed with max_results", async () => {
    const browser = getBrowser();
    browser.sessions.getRecentlyClosed.mockResolvedValue([{ tab: { url: "https://closed.com" } }]);
    const result = await handlers.sessions_closed({ max_results: 5 });
    expect(browser.sessions.getRecentlyClosed).toHaveBeenCalledWith({ maxResults: 5 });
    expect(result).toHaveLength(1);
  });
  it("returns recently closed with default filter", async () => {
    const browser = getBrowser();
    browser.sessions.getRecentlyClosed.mockResolvedValue([]);
    await handlers.sessions_closed({});
    expect(browser.sessions.getRecentlyClosed).toHaveBeenCalledWith({});
  });
});

describe("sessions_restore", () => {
  it("restores by session_id", async () => {
    const browser = getBrowser();
    browser.sessions.restore.mockResolvedValue({ tab: { url: "https://restored.com" } });
    const result = await handlers.sessions_restore({ session_id: "s1" });
    expect(browser.sessions.restore).toHaveBeenCalledWith("s1");
    expect(result).toBeDefined();
  });
  it("throws when no session_id (propagates error)", async () => {
    const browser = getBrowser();
    browser.sessions.restore.mockRejectedValue(new Error("invalid sessionId"));
    await expect(handlers.sessions_restore({})).rejects.toThrow("invalid sessionId");
  });
});

describe("session_restore_window", () => {
  it("restores window session", async () => {
    const browser = getBrowser();
    browser.sessions.getRecentlyClosed.mockResolvedValue([
      { window: { sessionId: "ws1" } },
    ]);
    browser.sessions.restore.mockResolvedValue({ window: { id: 99 } });
    const result = await handlers.session_restore_window({ session_id: "ws1" });
    expect(browser.sessions.restore).toHaveBeenCalledWith("ws1");
    expect(result.window.id).toBe(99);
  });
});

describe("session_restore_tab", () => {
  it("restores tab session", async () => {
    const browser = getBrowser();
    browser.sessions.getRecentlyClosed.mockResolvedValue([
      { tab: { sessionId: "ts1" } },
    ]);
    browser.sessions.restore.mockResolvedValue({ tab: { id: 7 } });
    const result = await handlers.session_restore_tab({ session_id: "ts1" });
    expect(browser.sessions.restore).toHaveBeenCalledWith("ts1");
    expect(result.tab.id).toBe(7);
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// DOWNLOADS
// ═══════════════════════════════════════════════════════════════════════════

describe("downloads_list", () => {
  it("returns all downloads when query empty", async () => {
    const browser = getBrowser();
    browser.downloads.search.mockResolvedValue([{ id: 1, filename: "a.zip" }]);
    const result = await handlers.downloads_list({});
    expect(browser.downloads.search).toHaveBeenCalledWith({ query: [] });
    expect(result).toHaveLength(1);
  });
  it("filters by query string", async () => {
    const browser = getBrowser();
    await handlers.downloads_list({ query: "invoice" });
    expect(browser.downloads.search).toHaveBeenCalledWith({ query: ["invoice"] });
  });
});

describe("downloads_start", () => {
  it("starts download", async () => {
    const browser = getBrowser();
    browser.downloads.download.mockResolvedValue(5);
    await handlers.downloads_start({ url: "https://file.zip", filename: "local.zip" });
    expect(browser.downloads.download).toHaveBeenCalledWith({ url: "https://file.zip", filename: "local.zip" });
  });
  it("starts download with save_as", async () => {
    const browser = getBrowser();
    await handlers.downloads_start({ url: "https://file.zip", save_as: true });
    expect(browser.downloads.download).toHaveBeenCalledWith({ url: "https://file.zip", saveAs: true });
  });
});

describe("downloads_cancel", () => {
  it("cancels download", async () => {
    const browser = getBrowser();
    await handlers.downloads_cancel({ download_id: 7 });
    expect(browser.downloads.cancel).toHaveBeenCalledWith(7);
  });
});

describe("downloads_pause", () => {
  it("pauses download", async () => {
    const browser = getBrowser();
    await handlers.downloads_pause({ download_id: 3 });
    expect(browser.downloads.pause).toHaveBeenCalledWith(3);
  });
});

describe("downloads_resume", () => {
  it("resumes download", async () => {
    const browser = getBrowser();
    await handlers.downloads_resume({ download_id: 3 });
    expect(browser.downloads.resume).toHaveBeenCalledWith(3);
  });
});
