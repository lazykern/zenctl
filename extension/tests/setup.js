/**
 * Test setup — runs before every test file.
 *
 * Installs a fresh browser.* mock on globalThis (at module eval time
 * so background.js can reference browser at its top level) and captures
 * it for per-test configuration.
 */
import { beforeEach } from "vitest";
import { createBrowserMock } from "./mocks/browser.js";

// Set at module level so dynamic import("./background.js") has browser
// available when the module body evaluates (e.g. `browser.runtime.getManifest()`).
let currentBrowser = createBrowserMock();
globalThis.browser = currentBrowser;

export function getBrowser() {
  return currentBrowser;
}

beforeEach(() => {
  currentBrowser = createBrowserMock();
  globalThis.browser = currentBrowser;
});
