/**
 * Handler tests: mods, boosts, folders, live_folders, and misc.
 */
import { describe, it, expect, beforeEach } from "vitest";
import { createBrowserMock, eventTarget } from "../mocks/browser.js";

let mod;
let browser;

beforeEach(async () => {
  browser = createBrowserMock();
  globalThis.browser = browser;
  const m = await import("../../src/background.js?reload=" + Date.now());
  mod = m;
});

function hasHandler(name) {
  if (typeof mod.handlers[name] !== "function") {
    throw new Error(`no handler "${name}" — export shim missing?`);
  }
  return mod.handlers[name];
}

/* ======================================================================
   Mods
   ====================================================================== */

describe("mods_list", () => {
  it("returns zen mod list", async () => {
    browser.zenChrome.modsList.mockResolvedValue([
      { id: "a", name: "Foo" },
    ]);
    const r = await mod.handlers.mods_list();
    expect(r).toEqual([{ id: "a", name: "Foo" }]);
  });
});

describe("mods_install", () => {
  it("rejects when neither mod_id nor url", async () => {
    await expect(mod.handlers.mods_install({})).rejects.toThrow("mod_id or url");
  });

  it("installs by mod_id", async () => {
    browser.zenChrome.modsInstall.mockResolvedValue({ installed: true });
    const r = await mod.handlers.mods_install({ mod_id: "abc" });
    expect(browser.zenChrome.modsInstall).toHaveBeenCalledWith("abc", "");
    expect(r).toEqual({ installed: true });
  });

  it("installs by url", async () => {
    browser.zenChrome.modsInstall.mockResolvedValue({ installed: true });
    await mod.handlers.mods_install({ url: "https://x.com/mod.zip" });
    expect(browser.zenChrome.modsInstall).toHaveBeenCalledWith("", "https://x.com/mod.zip");
  });
});

describe("mods_remove", () => {
  it("rejects without mod_id", async () => {
    await expect(mod.handlers.mods_remove({})).rejects.toThrow("mod_id required");
  });

  it("removes mod", async () => {
    browser.zenChrome.modsRemove.mockResolvedValue({ removed: true });
    await mod.handlers.mods_remove({ mod_id: "abc" });
    expect(browser.zenChrome.modsRemove).toHaveBeenCalledWith("abc");
  });
});

describe("mods_enable", () => {
  it("rejects without mod_id", async () => {
    await expect(mod.handlers.mods_enable({})).rejects.toThrow("mod_id required");
  });

  it("enables mod", async () => {
    browser.zenChrome.modsEnable.mockResolvedValue({ enabled: true });
    await mod.handlers.mods_enable({ mod_id: "abc" });
    expect(browser.zenChrome.modsEnable).toHaveBeenCalledWith("abc");
  });
});

describe("mods_disable", () => {
  it("disables mod", async () => {
    browser.zenChrome.modsDisable.mockResolvedValue({ disabled: true });
    await mod.handlers.mods_disable({ mod_id: "abc" });
    expect(browser.zenChrome.modsDisable).toHaveBeenCalledWith("abc");
  });
});

describe("mods_preferences", () => {
  it("rejects without mod_id", async () => {
    await expect(mod.handlers.mods_preferences({})).rejects.toThrow("mod_id required");
  });

  it("gets mod preferences", async () => {
    browser.zenChrome.modsPreferences.mockResolvedValue({
      preferences: [{ property: "enabled", type: "checkbox" }],
    });
    const r = await mod.handlers.mods_preferences({ mod_id: "abc" });
    expect(r.preferences).toHaveLength(1);
  });
});

describe("mods_set_preference", () => {
  it("rejects without mod_id", async () => {
    await expect(
      mod.handlers.mods_set_preference({})
    ).rejects.toThrow("mod_id required");
  });

  it("rejects without pref_name", async () => {
    await expect(
      mod.handlers.mods_set_preference({ mod_id: "a" })
    ).rejects.toThrow("pref_name required");
  });

  it("rejects without pref_value", async () => {
    await expect(
      mod.handlers.mods_set_preference({ mod_id: "a", pref_name: "x" })
    ).rejects.toThrow("pref_value required");
  });

  it("rejects when preference not found on mod", async () => {
    browser.zenChrome.modsPreferences.mockResolvedValue({
      preferences: [{ property: "other", type: "checkbox" }],
    });
    await expect(
      mod.handlers.mods_set_preference({ mod_id: "a", pref_name: "enabled", pref_value: true })
    ).rejects.toThrow('preference "enabled" not found');
  });

  it("rejects boolean pref with string value", async () => {
    browser.zenChrome.modsPreferences.mockResolvedValue({
      preferences: [{ property: "enabled", type: "checkbox" }],
    });
    await expect(
      mod.handlers.mods_set_preference({ mod_id: "a", pref_name: "enabled", pref_value: "yes" })
    ).rejects.toThrow('expects boolean, got string');
  });

  it("sets boolean preference", async () => {
    browser.zenChrome.modsPreferences.mockResolvedValue({
      preferences: [{ property: "enabled", type: "checkbox" }],
    });
    browser.zenPrefs.setPref.mockResolvedValue({ ok: true });
    const r = await mod.handlers.mods_set_preference({
      mod_id: "a", pref_name: "enabled", pref_value: false,
    });
    expect(browser.zenPrefs.setPref).toHaveBeenCalledWith("enabled", false);
    expect(r).toEqual({ ok: true });
  });

  it("sets string preference", async () => {
    browser.zenChrome.modsPreferences.mockResolvedValue({
      preferences: [{ property: "theme", type: "dropdown" }],
    });
    browser.zenPrefs.setPref.mockResolvedValue({ ok: true });
    await mod.handlers.mods_set_preference({
      mod_id: "a", pref_name: "theme", pref_value: "dark",
    });
    expect(browser.zenPrefs.setPref).toHaveBeenCalledWith("theme", "dark");
  });
});

/* ======================================================================
   Boosts
   ====================================================================== */

describe("boosts_list", () => {
  it("returns boosts", async () => {
    browser.zenChrome.boostsList.mockResolvedValue([
      { id: "b1", domain: "example.com" },
    ]);
    const r = await mod.handlers.boosts_list();
    expect(r).toHaveLength(1);
  });
});

describe("boosts_create", () => {
  it("rejects without domain", async () => {
    await expect(mod.handlers.boosts_create({})).rejects.toThrow("domain required");
  });

  it("creates boost for domain", async () => {
    browser.zenChrome.boostsCreate.mockResolvedValue({ id: "b2" });
    await mod.handlers.boosts_create({ domain: "example.com" });
    expect(browser.zenChrome.boostsCreate).toHaveBeenCalledWith("example.com");
  });
});

describe("boosts_delete", () => {
  it("rejects without domain + id", async () => {
    await expect(mod.handlers.boosts_delete({})).rejects.toThrow("domain + id");
    await expect(mod.handlers.boosts_delete({ domain: "x" })).rejects.toThrow("domain + id");
  });

  it("deletes boost", async () => {
    browser.zenChrome.boostsDelete.mockResolvedValue({ ok: true });
    await mod.handlers.boosts_delete({ domain: "x", id: "b1" });
    expect(browser.zenChrome.boostsDelete).toHaveBeenCalledWith("x", "b1");
  });
});

describe("boosts_activate", () => {
  it("activates boost", async () => {
    browser.zenChrome.boostsActivate.mockResolvedValue({ ok: true });
    await mod.handlers.boosts_activate({ domain: "x", id: "b1" });
    expect(browser.zenChrome.boostsActivate).toHaveBeenCalledWith("x", "b1");
  });
});

describe("boosts_toggle", () => {
  it("toggles boost", async () => {
    browser.zenChrome.boostsToggle.mockResolvedValue({ enabled: true });
    await mod.handlers.boosts_toggle({ domain: "x", id: "b1" });
    expect(browser.zenChrome.boostsToggle).toHaveBeenCalledWith("x", "b1");
  });
});

describe("boosts_update", () => {
  it("rejects without data_json", async () => {
    await expect(
      mod.handlers.boosts_update({ domain: "x", id: "b1" })
    ).rejects.toThrow("data_json required");
  });

  it("updates boost with JSON data", async () => {
    browser.zenChrome.boostsUpdate.mockResolvedValue({ ok: true });
    const data = JSON.stringify({ css: "body { color: red }" });
    await mod.handlers.boosts_update({ domain: "x", id: "b1", data_json: data });
    expect(browser.zenChrome.boostsUpdate).toHaveBeenCalledWith("x", "b1", data);
  });
});

/* ======================================================================
   Folders
   ====================================================================== */

describe("folders_list", () => {
  it("lists folders", async () => {
    browser.zenChrome.foldersList.mockResolvedValue([
      { id: "f1", label: "Dev" },
    ]);
    const r = await mod.handlers.folders_list();
    expect(r).toHaveLength(1);
  });
});

describe("folders_create", () => {
  it("creates folder", async () => {
    browser.zenChrome.foldersCreate.mockResolvedValue({ id: "f2" });
    await mod.handlers.folders_create({ label: "Dev", workspace_id: "ws-1" });
    expect(browser.zenChrome.foldersCreate).toHaveBeenCalledWith("Dev", "ws-1");
  });

  it("defaults label and workspace_id to empty", async () => {
    browser.zenChrome.foldersCreate.mockResolvedValue({ id: "f-d" });
    await mod.handlers.folders_create({});
    expect(browser.zenChrome.foldersCreate).toHaveBeenCalledWith("", "");
  });
});

describe("folders_delete", () => {
  it("rejects without folder_id", async () => {
    await expect(mod.handlers.folders_delete({})).rejects.toThrow("folder_id");
  });

  it("deletes folder", async () => {
    browser.zenChrome.foldersDelete.mockResolvedValue({ ok: true });
    await mod.handlers.folders_delete({ folder_id: "f1" });
    expect(browser.zenChrome.foldersDelete).toHaveBeenCalledWith("f1");
  });
});

describe("folders_rename", () => {
  it("renames folder", async () => {
    browser.zenChrome.foldersRename.mockResolvedValue({ ok: true });
    await mod.handlers.folders_rename({ folder_id: "f1", name: "New" });
    expect(browser.zenChrome.foldersRename).toHaveBeenCalledWith("f1", "New");
  });
});

describe("folders_collapse", () => {
  it("collapses folder", async () => {
    browser.zenChrome.foldersCollapse.mockResolvedValue({ ok: true });
    await mod.handlers.folders_collapse({ folder_id: "f1", collapsed: true });
    expect(browser.zenChrome.foldersCollapse).toHaveBeenCalledWith("f1", true);
  });

  it("expands folder (false)", async () => {
    browser.zenChrome.foldersCollapse.mockResolvedValue({ ok: true });
    await mod.handlers.folders_collapse({ folder_id: "f1", collapsed: false });
    expect(browser.zenChrome.foldersCollapse).toHaveBeenCalledWith("f1", false);
  });
});

describe("folders_add_tab", () => {
  it("rejects without folder_id", async () => {
    await expect(mod.handlers.folders_add_tab({})).rejects.toThrow("folder_id");
  });

  it("rejects when no urls or tab_ids", async () => {
    await expect(
      mod.handlers.folders_add_tab({ folder_id: "f1" })
    ).rejects.toThrow("tab_ids");
  });

  it("adds tab by URL", async () => {
    browser.zenChrome.foldersAddTab.mockResolvedValue({ ok: true });
    await mod.handlers.folders_add_tab({ folder_id: "f1", urls: ["https://a.com"] });
    expect(browser.zenChrome.foldersAddTab).toHaveBeenCalledWith("f1", '["https://a.com"]');
  });

  it("adds tab by tab_id (resolves URL)", async () => {
    browser.tabs.get.mockResolvedValue({ id: 1, url: "https://a.com" });
    browser.zenChrome.foldersAddTab.mockResolvedValue({ ok: true });
    await mod.handlers.folders_add_tab({ folder_id: "f1", tab_ids: [1] });
    expect(browser.tabs.get).toHaveBeenCalledWith(1);
    expect(browser.zenChrome.foldersAddTab).toHaveBeenCalledWith("f1", '["https://a.com"]');
  });
});

describe("folders_set_icon", () => {
  it("sets folder icon", async () => {
    browser.zenChrome.foldersSetIcon.mockResolvedValue({ ok: true });
    await mod.handlers.folders_set_icon({ folder_id: "f1", icon: "fingerprint" });
    expect(browser.zenChrome.foldersSetIcon).toHaveBeenCalledWith("f1", "fingerprint");
  });
});

describe("folders_create_subfolder", () => {
  it("creates subfolder", async () => {
    browser.zenChrome.foldersCreateSubfolder.mockResolvedValue({ id: "sub" });
    await mod.handlers.folders_create_subfolder({ parent_id: "f1", label: "Sub" });
    expect(browser.zenChrome.foldersCreateSubfolder).toHaveBeenCalledWith("f1", "Sub");
  });
});

describe("folders_unpack", () => {
  it("unpacks folder", async () => {
    browser.zenChrome.foldersUnpack.mockResolvedValue({ ok: true });
    await mod.handlers.folders_unpack({ folder_id: "f1" });
    expect(browser.zenChrome.foldersUnpack).toHaveBeenCalledWith("f1");
  });
});

describe("folders_unload", () => {
  it("unloads folder", async () => {
    browser.zenChrome.foldersUnload.mockResolvedValue({ ok: true });
    await mod.handlers.folders_unload({ folder_id: "f1" });
    expect(browser.zenChrome.foldersUnload).toHaveBeenCalledWith("f1");
  });
});

describe("folders_move_to_workspace", () => {
  it("moves folder to workspace", async () => {
    browser.zenChrome.foldersMoveToWorkspace.mockResolvedValue({ ok: true });
    await mod.handlers.folders_move_to_workspace({ folder_id: "f1", workspace_id: "ws-2" });
    expect(browser.zenChrome.foldersMoveToWorkspace).toHaveBeenCalledWith("f1", "ws-2");
  });
});

describe("folders_convert_to_workspace", () => {
  it("converts folder to workspace", async () => {
    browser.zenChrome.foldersConvertToWorkspace.mockResolvedValue({ uuid: "ws-3" });
    await mod.handlers.folders_convert_to_workspace({ folder_id: "f1" });
    expect(browser.zenChrome.foldersConvertToWorkspace).toHaveBeenCalledWith("f1");
  });
});

/* ======================================================================
   Live Folders
   ====================================================================== */

describe("live_folders_list", () => {
  it("lists live folders", async () => {
    browser.zenChrome.liveFoldersList.mockResolvedValue([{ id: "lf1" }]);
    const r = await mod.handlers.live_folders_list();
    expect(r).toHaveLength(1);
  });
});

describe("live_folders_create", () => {
  it("rejects without provider", async () => {
    await expect(
      mod.handlers.live_folders_create({})
    ).rejects.toThrow("provider required");
  });

  it("creates live folder", async () => {
    browser.zenChrome.liveFoldersCreate.mockResolvedValue({ id: "lf2" });
    await mod.handlers.live_folders_create({ provider: "reddit", url: "https://reddit.com/r/rust" });
    expect(browser.zenChrome.liveFoldersCreate).toHaveBeenCalledWith(
      "reddit", "https://reddit.com/r/rust", ""
    );
  });
});

describe("live_folders_delete", () => {
  it("deletes live folder", async () => {
    browser.zenChrome.liveFoldersDelete.mockResolvedValue({ ok: true });
    await mod.handlers.live_folders_delete({ folder_id: "lf1" });
    expect(browser.zenChrome.liveFoldersDelete).toHaveBeenCalledWith("lf1");
  });
});

describe("live_folders_refresh", () => {
  it("refreshes live folder", async () => {
    browser.zenChrome.liveFoldersRefresh.mockResolvedValue({ ok: true });
    await mod.handlers.live_folders_refresh({ folder_id: "lf1" });
    expect(browser.zenChrome.liveFoldersRefresh).toHaveBeenCalledWith("lf1");
  });
});

describe("live_folders_pause", () => {
  it("pauses live folder", async () => {
    browser.zenChrome.liveFoldersPause.mockResolvedValue({ ok: true });
    await mod.handlers.live_folders_pause({ folder_id: "lf1" });
    expect(browser.zenChrome.liveFoldersPause).toHaveBeenCalledWith("lf1");
  });
});

describe("live_folders_resume", () => {
  it("resumes live folder", async () => {
    browser.zenChrome.liveFoldersResume.mockResolvedValue({ ok: true });
    await mod.handlers.live_folders_resume({ folder_id: "lf1" });
    expect(browser.zenChrome.liveFoldersResume).toHaveBeenCalledWith("lf1");
  });
});

/* ======================================================================
   Misc handlers: compact_hide, glance_open, shortcuts_reset,
   tab_detach, tabs_screenshot, capabilities_probe
   ====================================================================== */

describe("compact_hide", () => {
  it("rejects without what", async () => {
    await expect(mod.handlers.compact_hide({})).rejects.toThrow("what required");
  });

  it("hides sidebar", async () => {
    browser.zenChrome.compactHide.mockResolvedValue({ ok: true });
    await mod.handlers.compact_hide({ what: "sidebar" });
    expect(browser.zenChrome.compactHide).toHaveBeenCalledWith("sidebar");
  });
});

describe("glance_open", () => {
  it("rejects without url", async () => {
    await expect(mod.handlers.glance_open({})).rejects.toThrow("url required");
  });

  it("opens glance", async () => {
    browser.zenChrome.glanceOpen.mockResolvedValue({ ok: true });
    await mod.handlers.glance_open({ url: "https://x.com" });
    expect(browser.zenChrome.glanceOpen).toHaveBeenCalledWith("https://x.com");
  });
});

describe("shortcuts_reset", () => {
  it("resets shortcuts", async () => {
    browser.zenChrome.shortcutsReset.mockResolvedValue({ ok: true });
    await mod.handlers.shortcuts_reset();
    expect(browser.zenChrome.shortcutsReset).toHaveBeenCalled();
  });
});

describe("tabs_screenshot", () => {
  it("captures visible area in PNG", async () => {
    browser.tabs.query.mockResolvedValue([{ id: 1, url: "https://a.com" }]);
    browser.tabs.captureTab.mockResolvedValue("data:image/png;base64,ABC");
    const r = await mod.handlers.tabs_screenshot({ tab_id: 1 });
    expect(r.tab_id).toBe(1);
    expect(r.format).toBe("png");
    expect(r.data_url).toContain("data:image/png");
  });

  it("captures visible area in JPEG", async () => {
    browser.tabs.query.mockResolvedValue([{ id: 1, url: "https://a.com" }]);
    browser.tabs.captureTab.mockResolvedValue("data:image/jpeg;base64,ABC");
    const r = await mod.handlers.tabs_screenshot({ tab_id: 1, format: "jpeg", quality: 80 });
    expect(r.format).toBe("jpeg");
  });
});

describe("tab_detach", () => {
  it("detaches tab into new window", async () => {
    browser.tabs.get.mockResolvedValue({ id: 1, url: "https://a.com", incognito: false });
    browser.windows.create.mockResolvedValue({ id: 42 });
    browser.tabs.remove.mockResolvedValue(undefined);
    browser.tabs.query.mockResolvedValue([{ id: 2 }]);

    const r = await mod.handlers.tab_detach({ tab_id: 1 });
    expect(r.tab_id).toBe(2);
    expect(r.window_id).toBe(42);
    expect(r.url).toBe("https://a.com");
    expect(browser.windows.create).toHaveBeenCalledWith({
      url: "https://a.com", incognito: false,
    });
  });

  it("rejects without tab_id", async () => {
    await expect(mod.handlers.tab_detach({})).rejects.toThrow("tab_id required");
  });
});

describe("capabilities_probe", () => {
  it("probes experiment API availability", async () => {
    // compactToggle is available via mock
    browser.zenChrome = {
      ...browser.zenChrome,
      compactToggle: async () => ({}),
    };
    browser.zenPrefs = {
      ...browser.zenPrefs,
    };
    const r = await mod.handlers.capabilities_probe();
    expect(r.zen_chrome).toBe(true);
    expect(r.zen_prefs).toBe(true);
  });
});

describe("essentials_reset", () => {
  it("rejects without urls or tab_ids", async () => {
    await expect(
      mod.handlers.essentials_reset({})
    ).rejects.toThrow("tab_ids");
  });

  it("resets essentials by URL", async () => {
    browser.zenChrome.essentialsReset.mockResolvedValue({ ok: true });
    await mod.handlers.essentials_reset({ urls: ["https://a.com"] });
    expect(browser.zenChrome.essentialsReset).toHaveBeenCalledWith('["https://a.com"]');
  });
});

describe("essentials_replace_url", () => {
  it("replaces essentials URL", async () => {
    browser.zenChrome.essentialsReplaceUrl.mockResolvedValue({ ok: true });
    await mod.handlers.essentials_replace_url({ urls: ["https://a.com"] });
    expect(browser.zenChrome.essentialsReplaceUrl).toHaveBeenCalledWith('["https://a.com"]');
  });
});
