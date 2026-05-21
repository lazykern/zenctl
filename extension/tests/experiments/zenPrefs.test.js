/**
 * Tests: zenPrefs experiment API (4 methods).
 *
 * Uses loadExperiment() to evaluate the Firefox experiment file in a
 * CommonJS-like context where `this.Foo = class ...` becomes module.exports.
 */
import { describe, it, expect, beforeAll } from "vitest";
import { loadExperiment } from "../load-experiment.js";
import { fileURLToPath } from "url";
import { dirname, resolve } from "path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const zenPrefsPath = resolve(__dirname, "../../api/zenPrefs.js");

function createPrefService() {
  const store = new Map();
  const defaultStores = new Map();

  return {
    PREF_BOOL: 1,
    PREF_INT: 2,
    PREF_STRING: 3,

    prefHasUserValue(name) {
      return store.has(name);
    },

    getPrefType(name) {
      const val = store.has(name) ? store.get(name) : defaultStores.get(name);
      if (val === undefined) throw new Error(`unknown pref: ${name}`);
      if (typeof val === "boolean") return 1;
      if (typeof val === "number" && Number.isInteger(val)) return 2;
      if (typeof val === "string") return 3;
      return 0;
    },

    getBoolPref(name) {
      const val = store.has(name) ? store.get(name) : defaultStores.get(name);
      if (typeof val !== "boolean") throw new Error(`not bool: ${name}`);
      return val;
    },
    getIntPref(name) {
      const val = store.has(name) ? store.get(name) : defaultStores.get(name);
      if (typeof val !== "number" || !Number.isInteger(val)) throw new Error(`not int: ${name}`);
      return val;
    },
    getStringPref(name) {
      const val = store.has(name) ? store.get(name) : defaultStores.get(name);
      if (typeof val !== "string") throw new Error(`not string: ${name}`);
      return val;
    },

    setBoolPref(name, val) { store.set(name, val); },
    setIntPref(name, val) { store.set(name, val); },
    setStringPref(name, val) { store.set(name, val); },

    clearUserPref(name) { store.delete(name); },

    getBranch(root) {
      const prefix = root || "";
      const allNames = new Set([...store.keys(), ...defaultStores.keys()]);
      return {
        getChildList(_) {
          return [...allNames]
            .filter((n) => n.startsWith(prefix) && n !== prefix)
            .map((n) => n.slice(prefix.length));
        },
      };
    },

    // test helpers
    _setDefault(name, value) { defaultStores.set(name, value); },
    _clear() { store.clear(); defaultStores.clear(); },
  };
}

let prefs;
let api;

beforeAll(() => {
  prefs = createPrefService();

  class MockExtensionAPI {
    getAPI(_context) {
      return {};
    }
  }

  const ZenPrefsClass = loadExperiment(zenPrefsPath, {
    ExtensionAPI: MockExtensionAPI,
    Services: { prefs },
  });
  const instance = new ZenPrefsClass();
  api = instance.getAPI().zenPrefs;
});

describe("zenPrefs.getPref", () => {
  it("returns null for unset pref", async () => {
    prefs._setDefault("test.bool", true);
    const result = await api.getPref("test.bool");
    expect(result).toEqual({ type: "bool", value: true, has_user_value: false });
  });

  it("returns bool pref with user value", async () => {
    prefs.setBoolPref("my.feature", true);
    const result = await api.getPref("my.feature");
    expect(result).toEqual({ type: "bool", value: true, has_user_value: true });
  });

  it("returns int pref", async () => {
    prefs.setIntPref("my.count", 42);
    const result = await api.getPref("my.count");
    expect(result).toEqual({ type: "int", value: 42, has_user_value: true });
  });

  it("returns string pref", async () => {
    prefs.setStringPref("my.name", "zenctl");
    const result = await api.getPref("my.name");
    expect(result).toEqual({ type: "string", value: "zenctl", has_user_value: true });
  });

  it("returns null for nonexistent pref", async () => {
    const result = await api.getPref("does.not.exist");
    expect(result).toBeNull();
  });
});

describe("zenPrefs.setPref", () => {
  it("sets bool pref and returns result", async () => {
    const result = await api.setPref("set.bool", true);
    expect(result).toEqual({ type: "bool", value: true, has_user_value: true });
  });

  it("sets int pref", async () => {
    const result = await api.setPref("set.int", 100);
    expect(result).toEqual({ type: "int", value: 100, has_user_value: true });
  });

  it("sets string pref", async () => {
    const result = await api.setPref("set.str", "hello");
    expect(result).toEqual({ type: "string", value: "hello", has_user_value: true });
  });

  it("throws on unsupported type", async () => {
    await expect(api.setPref("bad", {})).rejects.toThrow("unsupported pref value type");
  });
});

describe("zenPrefs.clearUserPref", () => {
  it("clears user value and returns true", async () => {
    prefs.setStringPref("temp.val", "clear me");
    expect(prefs.prefHasUserValue("temp.val")).toBe(true);
    const result = await api.clearUserPref("temp.val");
    expect(result).toBe(true);
    expect(prefs.prefHasUserValue("temp.val")).toBe(false);
  });
});

describe("zenPrefs.listPrefs", () => {
  it("lists prefs by prefix", async () => {
    prefs.setStringPref("app.name", "zenctl");
    prefs.setIntPref("app.version", 1);
    prefs.setBoolPref("other.flag", false);

    const result = await api.listPrefs("app.");
    expect(result).toHaveLength(2);

    const names = result.map((r) => r.name).sort();
    expect(names).toEqual(["app.name", "app.version"]);
  });

  it("lists all prefs without prefix", async () => {
    prefs.setStringPref("x.a", "1");
    prefs.setIntPref("y.b", 2);
    const result = await api.listPrefs("");
    expect(result.length).toBeGreaterThanOrEqual(2);
  });

  it("returns empty array for no matches", async () => {
    const result = await api.listPrefs("zzz.");
    expect(result).toEqual([]);
  });
});
