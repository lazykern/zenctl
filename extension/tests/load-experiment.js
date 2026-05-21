/**
 * Loads a Firefox WebExtension Experiment API file (.js) in Node.js.
 *
 * Experiment files use `this.apiName = class extends ExtensionAPI { ... }`.
 * In Node.js ESM, `this` at module scope is undefined, so these assignments
 * are lost. This helper reads the file, wraps it in a CommonJS context, and
 * returns the exported class.
 *
 * IMPORTANT: Sets globals on globalThis and leaves them there. The loaded
 * class instance's getAPI() closure captures these globals. Tests that load
 * multiple experiment files should use a dedicated setup file.
 *
 * Usage:
 *   const zenPrefs = loadExperiment("/abs/path/to/zenPrefs.js", { ExtensionAPI, Services });
 *   const instance = new zenPrefs();
 *   const api = instance.getAPI().zenPrefs;
 */
import { readFileSync } from "fs";
import { createRequire } from "module";

const _require = createRequire(import.meta.url);

export function loadExperiment(absPath, globals = {}) {
  let source = readFileSync(absPath, "utf-8");

  // Replace `this.Foo = class extends ExtensionAPI` with `module.exports.Foo = class extends globalThis.__ExtensionAPI`
  source = source.replace(
    /^\s*this\.(\w+)\s*=\s*class\s+extends\s+ExtensionAPI(\s*\{)/m,
    "module.exports.$1 = class extends globalThis.__ExtensionAPI$2"
  );

  // Make globals available on globalThis so the class body + getAPI closures see them.
  for (const [key, val] of Object.entries(globals)) {
    globalThis[key] = val;
  }
  if (globals.ExtensionAPI) {
    globalThis.__ExtensionAPI = globals.ExtensionAPI;
  }

  const wrappedSource = `
const module = { exports: {} };
const exports = module.exports;
${source}
return module.exports;
`;
  const fn = new Function("require", wrappedSource);
  const result = fn(_require);

  // The class instance's getAPI() closure captures the globals set above.
  // Do NOT clean them up until the test suite tears down.

  const keys = Object.keys(result || {});
  return keys.length > 0 ? result[keys[0]] : result;
}
