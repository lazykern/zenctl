/**
 * Tests: UI/Zen handlers (compact, workspace, glance, split, urlbar,
 * essentials, mods, boosts, folders, live_folders, share).
 *
 * Most handlers delegate to browser.zenChrome experiment APIs. Tests verify
 * parameter validation, requireChromeApi guard, and correct delegation.
 */
import { describe, it, expect, beforeAll } from "vitest";
import { getBrowser } from "../setup.js";

let handlers = {};
beforeAll(async () => {
  handlers = (await import("../../src/background.js")).handlers;
});

// Helper: ensure browser.zenChrome exists (mock already has it from setup)
function hasChrome(b) {
  return !!(b.zenChrome && b.zenChrome.compactToggle);
}

// ═══════════════════════════════════════════════════════════════════════════
// COMPACT MODE
// ═══════════════════════════════════════════════════════════════════════════

describe("compact_toggle", () => {
  it("toggles compact mode", async () => {
    const b = getBrowser();
    b.zenChrome.compactToggle.mockResolvedValue({ enabled: true });
    const result = await handlers.compact_toggle();
    expect(b.zenChrome.compactToggle).toHaveBeenCalled();
    expect(result.enabled).toBe(true);
  });
});

describe("compact_set", () => {
  it("throws when value not boolean", async () => {
    await expect(handlers.compact_set({ value: "yes" })).rejects.toThrow("value: bool required");
  });
  it("sets compact mode", async () => {
    const b = getBrowser();
    b.zenChrome.compactSet.mockResolvedValue({ enabled: false });
    const result = await handlers.compact_set({ value: false });
    expect(b.zenChrome.compactSet).toHaveBeenCalledWith(false);
    expect(result.enabled).toBe(false);
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// WORKSPACES
// ═══════════════════════════════════════════════════════════════════════════

describe("workspace_switch", () => {
  it("throws when uuid missing", async () => {
    await expect(handlers.workspace_switch({})).rejects.toThrow("uuid required");
  });
  it("switches to workspace", async () => {
    const b = getBrowser();
    b.zenChrome.workspaceSwitch.mockResolvedValue({ switched: true });
    await handlers.workspace_switch({ uuid: "abc123" });
    expect(b.zenChrome.workspaceSwitch).toHaveBeenCalledWith("abc123");
  });
});

describe("workspace_list", () => {
  it("lists workspaces with prefs", async () => {
    const b = getBrowser();
    b.zenChrome.workspacesList.mockResolvedValue({ active: "ws1", workspaces: [] });
    b.zenPrefs.listPrefs.mockResolvedValue([]);
    const result = await handlers.workspace_list();
    expect(result.workspaces).toEqual([]);
    expect(result.prefs).toEqual([]);
  });
});

describe("workspace_unload", () => {
  it("unloads workspace", async () => {
    const b = getBrowser();
    await handlers.workspace_unload({ uuid: "ws1" });
    expect(b.zenChrome.workspaceUnload).toHaveBeenCalledWith("ws1", false);
  });
  it("unloads with empty uuid when missing", async () => {
    const b = getBrowser();
    await handlers.workspace_unload({});
    expect(b.zenChrome.workspaceUnload).toHaveBeenCalledWith("", false);
  });
});

describe("workspace_unload_all", () => {
  it("unloads all except given uuid", async () => {
    const b = getBrowser();
    await handlers.workspace_unload_all({ except_uuid: "keep" });
    expect(b.zenChrome.workspaceUnload).toHaveBeenCalledWith("keep", true);
  });
  it("unloads all when no except_uuid", async () => {
    const b = getBrowser();
    await handlers.workspace_unload_all({});
    expect(b.zenChrome.workspaceUnload).toHaveBeenCalledWith("", true);
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// GLANCE
// ═══════════════════════════════════════════════════════════════════════════

describe("glance_close", () => {
  it("closes glance with optional tab activation", async () => {
    const b = getBrowser();
    b.zenChrome.glanceClose.mockResolvedValue(undefined);
    await handlers.glance_close({ tab_id: 5 });
    expect(b.tabs.update).toHaveBeenCalledWith(5, { active: true });
    expect(b.zenChrome.glanceClose).toHaveBeenCalled();
  });
  it("closes glance without tab_id", async () => {
    const b = getBrowser();
    await handlers.glance_close({});
    expect(b.tabs.update).not.toHaveBeenCalled();
  });
});

describe("glance_expand", () => {
  it("expands glance", async () => {
    const b = getBrowser();
    b.zenChrome.glanceExpand.mockResolvedValue(true);
    await handlers.glance_expand({});
    expect(b.zenChrome.glanceExpand).toHaveBeenCalled();
  });
});

describe("glance_list", () => {
  it("lists glance tabs", async () => {
    const b = getBrowser();
    b.zenChrome.glanceList.mockResolvedValue([]);
    expect(await handlers.glance_list()).toEqual([]);
  });
});

describe("glance_close_all", () => {
  it("closes all glances", async () => {
    const b = getBrowser();
    await handlers.glance_close_all();
    expect(b.zenChrome.glanceCloseAll).toHaveBeenCalled();
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// SPLIT VIEW
// ═══════════════════════════════════════════════════════════════════════════

describe("split_view_create", () => {
  it("creates split view with tab_ids", async () => {
    const b = getBrowser();
    b.tabs.get.mockResolvedValue({ id: 1, url: "https://a.com" });
    b.zenChrome.splitViewCreate.mockResolvedValue(true);
    await handlers.split_view_create({ tab_ids: [1], grid_type: "row" });
    expect(b.zenChrome.splitViewCreate).toHaveBeenCalledWith(
      JSON.stringify(["https://a.com"]), "row",
    );
  });
  it("creates with default grid_type", async () => {
    const b = getBrowser();
    b.zenChrome.splitViewCreate.mockResolvedValue(true);
    await handlers.split_view_create({});
    expect(b.zenChrome.splitViewCreate).toHaveBeenCalledWith("[]", "grid");
  });
});

describe("split_unsplit", () => {
  it("unsplits current view", async () => {
    const b = getBrowser();
    b.zenChrome.splitUnsplit.mockResolvedValue(true);
    await handlers.split_unsplit();
    expect(b.zenChrome.splitUnsplit).toHaveBeenCalled();
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// URLBAR
// ═══════════════════════════════════════════════════════════════════════════

describe("urlbar_search", () => {
  it("searches via urlbar", async () => {
    const b = getBrowser();
    await handlers.urlbar_search({ query: "test", submit: true });
    expect(b.zenChrome.urlbarSearch).toHaveBeenCalledWith("test", true);
  });
});

describe("urlbar_close", () => {
  it("closes urlbar", async () => {
    const b = getBrowser();
    await handlers.urlbar_close();
    expect(b.zenChrome.urlbarClose).toHaveBeenCalled();
  });
});

describe("urlbar_actions_list", () => {
  it("lists urlbar actions", async () => {
    const b = getBrowser();
    b.zenChrome.urlbarActionsList.mockResolvedValue([{ id: "a1" }]);
    const result = await handlers.urlbar_actions_list();
    expect(result).toHaveLength(1);
  });
});

describe("urlbar_actions_run", () => {
  it("runs a urlbar action", async () => {
    const b = getBrowser();
    await handlers.urlbar_actions_run({ action: "a1" });
    expect(b.zenChrome.urlbarActionsRun).toHaveBeenCalledWith("a1");
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// SHARE
// ═══════════════════════════════════════════════════════════════════════════

describe("share_can", () => {
  it("returns share capability", async () => {
    const b = getBrowser();
    b.zenChrome.shareCan.mockResolvedValue(true);
    const result = await handlers.share_can();
    expect(result).toBe(true);
  });
});

describe("share", () => {
  it("shares url + title + text", async () => {
    const b = getBrowser();
    b.zenChrome.share.mockResolvedValue({ success: true });
    const result = await handlers.share({ url: "https://x.com", title: "X", text: "check" });
    expect(b.zenChrome.share).toHaveBeenCalledWith("https://x.com", "X", "check");
    expect(result.success).toBe(true);
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// WORKSPACE CRUD
// ═══════════════════════════════════════════════════════════════════════════

describe("workspace_create", () => {
  it("creates workspace", async () => {
    const b = getBrowser();
    b.zenChrome.workspaceCreate.mockResolvedValue({ uuid: "new-ws" });
    await handlers.workspace_create({ name: "New", icon: "star" });
    expect(b.zenChrome.workspaceCreate).toHaveBeenCalledWith("New", "star");
  });
});

describe("workspace_remove", () => {
  it("removes workspace", async () => {
    const b = getBrowser();
    await handlers.workspace_remove({ uuid: "old" });
    expect(b.zenChrome.workspaceRemove).toHaveBeenCalledWith("old");
  });
});

describe("workspace_rename", () => {
  it("renames workspace", async () => {
    const b = getBrowser();
    await handlers.workspace_rename({ uuid: "ws1", name: "Fun" });
    expect(b.zenChrome.workspaceRename).toHaveBeenCalledWith("ws1", "Fun");
  });
});

describe("workspace_set_icon", () => {
  it("sets workspace icon", async () => {
    const b = getBrowser();
    await handlers.workspace_set_icon({ uuid: "ws1", icon: "fire" });
    expect(b.zenChrome.workspaceSetIcon).toHaveBeenCalledWith("ws1", "fire");
  });
});

describe("workspace_set_container", () => {
  it("sets workspace container", async () => {
    const b = getBrowser();
    await handlers.workspace_set_container({ uuid: "ws1", cookie_store_id: "c1" });
    expect(b.zenChrome.workspaceSetContainer).toHaveBeenCalledWith("ws1", "c1");
  });
});

describe("workspace_reorder", () => {
  it("reorders workspace", async () => {
    const b = getBrowser();
    await handlers.workspace_reorder({ uuid: "ws1", index: 2 });
    expect(b.zenChrome.workspaceReorder).toHaveBeenCalledWith("ws1", 2);
  });
});

describe("workspace_move_tab", () => {
  it("moves tabs to workspace", async () => {
    const b = getBrowser();
    b.tabs.get.mockResolvedValueOnce({ id: 1, url: "https://a.com" });
    await handlers.workspace_move_tab({ uuid: "dest", tab_ids: [1] });
    expect(b.zenChrome.workspaceMoveTab).toHaveBeenCalledWith(
      "dest", expect.stringContaining("https://a.com")
    );
  });
  it("moves urls directly", async () => {
    const b = getBrowser();
    await handlers.workspace_move_tab({ uuid: "dest", urls: ["https://b.com"] });
    expect(b.zenChrome.workspaceMoveTab).toHaveBeenCalledWith(
      "dest", expect.stringContaining("https://b.com")
    );
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// ESSENTIALS
// ═══════════════════════════════════════════════════════════════════════════

describe("essentials_list", () => {
  it("lists essentials", async () => {
    const b = getBrowser();
    b.zenChrome.essentialsList.mockResolvedValue([]);
    expect(await handlers.essentials_list()).toEqual([]);
  });
});

describe("essentials_add", () => {
  it("adds by tab_ids", async () => {
    const b = getBrowser();
    b.tabs.get.mockResolvedValue({ id: 5, url: "https://a.com" });
    await handlers.essentials_add({ tab_ids: [5] });
    expect(b.zenChrome.essentialsAdd).toHaveBeenCalledWith(
      expect.stringContaining("https://a.com")
    );
  });
  it("adds by urls", async () => {
    const b = getBrowser();
    await handlers.essentials_add({ urls: ["https://b.com", "https://c.com"] });
    expect(b.zenChrome.essentialsAdd).toHaveBeenCalledWith(
      expect.stringContaining("https://b.com")
    );
  });
});

describe("essentials_remove", () => {
  it("removes by tab_ids", async () => {
    const b = getBrowser();
    b.tabs.get.mockResolvedValue({ id: 1, url: "https://a.com" });
    await handlers.essentials_remove({ tab_ids: [1], unpin: true });
    expect(b.zenChrome.essentialsRemove).toHaveBeenCalledWith(
      expect.stringContaining("https://a.com"), true
    );
  });
  it("removes by urls without unpin", async () => {
    const b = getBrowser();
    await handlers.essentials_remove({ urls: ["https://x.com"] });
    expect(b.zenChrome.essentialsRemove).toHaveBeenCalledWith(
      expect.stringContaining("https://x.com"), true
    );
  });
});
