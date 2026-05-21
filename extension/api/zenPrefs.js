"use strict";

// Privileged WebExtension Experiment that exposes Services.prefs to the
// zenctl background page. Runs in the main (parent) process.
//
// `Services` and `ChromeUtils` are globals in privileged contexts; no import
// needed on Firefox 100+.

/* global ExtensionAPI, Services */

this.zenPrefs = class extends ExtensionAPI {
  getAPI(_context) {
    const prefs = Services.prefs;

    function readPref(name) {
      const type = prefs.getPrefType(name);
      switch (type) {
        case prefs.PREF_BOOL:
          return { type: "bool", value: prefs.getBoolPref(name), has_user_value: prefs.prefHasUserValue(name) };
        case prefs.PREF_INT:
          return { type: "int", value: prefs.getIntPref(name), has_user_value: prefs.prefHasUserValue(name) };
        case prefs.PREF_STRING:
          return { type: "string", value: prefs.getStringPref(name), has_user_value: prefs.prefHasUserValue(name) };
        default:
          return null;
      }
    }

    return {
      zenPrefs: {
        async getPref(name) {
          try {
            return readPref(name);
          } catch (e) {
            return null;
          }
        },

        async setPref(name, value) {
          if (typeof value === "boolean") {
            prefs.setBoolPref(name, value);
          } else if (typeof value === "number" && Number.isInteger(value)) {
            prefs.setIntPref(name, value);
          } else if (typeof value === "string") {
            prefs.setStringPref(name, value);
          } else {
            throw new Error(`unsupported pref value type: ${typeof value}`);
          }
          return readPref(name);
        },

        async clearUserPref(name) {
          prefs.clearUserPref(name);
          return true;
        },

        async listPrefs(prefix) {
          const root = prefix || "";
          const branch = prefs.getBranch(root);
          const children = branch.getChildList("");
          const out = [];
          for (const child of children) {
            const full = root + child;
            try {
              const entry = readPref(full);
              if (entry) {
                entry.name = full;
                out.push(entry);
              }
            } catch (e) {
              // skip unreadable
            }
          }
          out.sort((a, b) => a.name.localeCompare(b.name));
          return out;
        },
      },
    };
  }
};
