"use strict";

// Native messaging host name — must match the "name" field in the manifest
// JSON that `zenctl install` writes, and the string in install.rs.
const HOST_NAME = "zenctl";
const PROTOCOL_VERSION = 1;
const CLIENT_VERSION = browser.runtime.getManifest().version;

let port = null;
let nextId = 1;
let reconnectDelay = 1_000;
let reconnectTimer = null;

// Files the host bundles into its binary, in the same order as
// install.rs::EXTENSION_FILES. computeFingerprint() hashes
// `path \0 content \0` for each in this order and the result must match
// install::extension_fingerprint() exactly. Used to detect a stale extension
// after a host upgrade.
const FINGERPRINT_FILES = [
  "manifest-basic.json",
  "manifest-privileged.json",
  "api/zenChrome.js",
  "api/zenChrome.json",
  "api/zenPrefs.js",
  "api/zenPrefs.json",
  "src/background.js",
  "src/options.html",
  "src/options.js",
];

async function computeFingerprint() {
  const enc = new TextEncoder();
  const parts = [];
  for (const rel of FINGERPRINT_FILES) {
    parts.push(enc.encode(rel));
    parts.push(new Uint8Array([0]));
    try {
      const text = await fetch(browser.runtime.getURL(rel)).then((r) => r.text());
      parts.push(enc.encode(text));
    } catch (e) {
      parts.push(enc.encode(`<missing:${e?.message ?? e}>`));
    }
    parts.push(new Uint8Array([0]));
  }
  const total = parts.reduce((s, p) => s + p.length, 0);
  const merged = new Uint8Array(total);
  let off = 0;
  for (const p of parts) {
    merged.set(p, off);
    off += p.length;
  }
  const digest = await crypto.subtle.digest("SHA-256", merged);
  return Array.from(new Uint8Array(digest))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

let extensionFingerprint = null;
computeFingerprint()
  .then((fp) => {
    extensionFingerprint = fp;
    console.log(`[zenctl] extension fingerprint ${fp.slice(0, 12)}…`);
  })
  .catch((e) => {
    console.warn("[zenctl] fingerprint failed:", e?.message ?? e);
  });

// Verbose request/response logging. Toggle with `zenctl ext debug on|off`,
// persisted in extension storage so it survives restarts.
const DEBUG_KEY = "zenctl_debug";
let DEBUG = false;
browser.storage.local.get(DEBUG_KEY).then((s) => {
  DEBUG = !!s[DEBUG_KEY];
  if (DEBUG) console.log("[zenctl] debug logging enabled");
});
browser.storage.onChanged.addListener((changes, area) => {
  if (area === "local" && DEBUG_KEY in changes) {
    DEBUG = !!changes[DEBUG_KEY].newValue;
    console.log(`[zenctl] debug logging ${DEBUG ? "enabled" : "disabled"}`);
  }
});

// ---------------------------------------------------------------------------
// Connection management
// ---------------------------------------------------------------------------

function scheduleReconnect() {
  if (reconnectTimer) return;
  const delay = reconnectDelay;
  reconnectDelay = Math.min(reconnectDelay * 2, 30_000);
  console.log(`[zenctl] reconnecting in ${delay}ms`);
  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    connect();
  }, delay);
}

function connect() {
  try {
    port = browser.runtime.connectNative(HOST_NAME);
  } catch (e) {
    console.warn("[zenctl] connectNative failed:", e.message ?? e);
    scheduleReconnect();
    return;
  }

  port.onMessage.addListener((msg) => {
    if (msg.type === "request") {
      // The host is forwarding a CLI request for us to execute.
      handleRequest(msg).catch((err) => {
        console.error("[zenctl] handler error", err);
        sendError(msg.id, "internal", String(err));
      });
    } else if (msg.type === "response" && msg.data?.host_version !== undefined) {
      // Hello ack.
      if (msg.data.protocol_version !== PROTOCOL_VERSION) {
        console.error(
          `[zenctl] protocol mismatch: host=${msg.data.protocol_version} ext=${PROTOCOL_VERSION}`
        );
        port.disconnect();
        return;
      }
      console.log(`[zenctl] connected to zenctl host ${msg.data.host_version}`);
      reconnectDelay = 1_000;
    }
  });

  port.onDisconnect.addListener(() => {
    const err = browser.runtime.lastError;
    if (err) console.warn("[zenctl] disconnected:", err.message);
    else console.log("[zenctl] disconnected");
    port = null;
    scheduleReconnect();
  });

  // Handshake: identify ourselves to the host. The fingerprint may not be
  // ready yet (it's computed lazily on extension load) — wait briefly so the
  // host can flag a stale extension on the very first connection.
  sendHelloWhenReady();
}

async function sendHelloWhenReady() {
  const deadline = Date.now() + 2000;
  while (extensionFingerprint === null && Date.now() < deadline) {
    await new Promise((r) => setTimeout(r, 50));
  }
  if (!port) return;
  port.postMessage({
    type: "request",
    id: nextId++,
    method: "hello",
    params: {
      client_version: CLIENT_VERSION,
      extension_fingerprint: extensionFingerprint,
    },
  });
}

function sendOk(id, data) {
  if (!port) return;
  port.postMessage({ type: "response", id, data });
}

function sendError(id, code, message) {
  if (!port) return;
  const error = message ? { code, message } : { code };
  port.postMessage({ type: "response", id, error });
}

function emitEvent(topic, payload) {
  if (!port) return;
  port.postMessage({ type: "event", topic, payload });
}

// ---------------------------------------------------------------------------
// Page helpers
// ---------------------------------------------------------------------------

async function activeTabId(windowId) {
  const q = { active: true };
  if (windowId !== undefined) q.windowId = windowId;
  else q.currentWindow = true;
  const tabs = await browser.tabs.query(q);
  if (!tabs.length) throw new Error("no active tab");
  return tabs[0].id;
}

function realPageUrl(url) {
  return !!url && !url.startsWith("about:") && !url.startsWith("moz-extension:");
}

async function targetTabId(params = {}) {
  const target = params.target ?? params;
  if (target.tab_id !== undefined && target.tab_id !== null) return target.tab_id;

  const hasSelector = target.tab_index !== undefined || target.url_contains || target.title_contains;

  let tabs;
  if (target.window_id !== undefined && target.window_id !== null) {
    tabs = await browser.tabs.query({ windowId: target.window_id });
  } else if (target.active) {
    // active without window_id -> scope to current window
    tabs = await browser.tabs.query({ currentWindow: true });
  } else if (hasSelector) {
    tabs = await queryAllTabsSafe();
  } else {
    // No selector at all -> prefer the active tab from a focused/normal
    // window with a real URL. Falls back to any active tab.
    const wins = await browser.windows.getAll({ populate: true, windowTypes: ["normal"] });
    const actives = wins
      .map((w) => (w.tabs || []).find((t) => t.active))
      .filter(Boolean);
    const real = actives.find((t) => realPageUrl(t.url));
    if (real) return real.id;
    if (actives.length) return actives[0].id;
    throw new Error("no active tab in any normal window");
  }

  const matches = tabs.filter((tab) => {
    if (target.active && !tab.active) return false;
    if (target.tab_index !== undefined && target.tab_index !== null && tab.index !== target.tab_index) return false;
    if (target.url_contains && !(tab.url || "").includes(target.url_contains)) return false;
    if (target.title_contains && !(tab.title || "").includes(target.title_contains)) return false;
    return true;
  });
  if (matches.length) {
    // Prefer the active tab among matches; then any with a real URL.
    const act = matches.find((t) => t.active);
    if (act) return act.id;
    const real = matches.find((t) => realPageUrl(t.url));
    return (real ?? matches[0]).id;
  }

  if (target.active || (target.window_id !== undefined && target.window_id !== null)) {
    return await activeTabId(target.window_id);
  }
  return await activeTabId();
}

// Some Zen windows (popups, internal panels) make `browser.tabs.query({})`
// reject with "An unexpected error occurred". Walk windows and accumulate
// per-window results, skipping ones that fail.
async function queryAllTabsSafe() {
  let windows;
  try {
    windows = await browser.windows.getAll({ populate: false });
  } catch (e) {
    return await browser.tabs.query({ currentWindow: true });
  }
  const out = [];
  for (const w of windows) {
    try {
      // Normal (visible) tabs.
      const visible = await browser.tabs.query({ windowId: w.id });
      out.push(...visible);
      // Hidden tabs — tabs in non-active workspaces are marked hidden
      // by Zen, so `tabs.query()` excludes them by default. Include
      // them so `tabs list` / `tabs find` work cross-workspace.
      try {
        const hidden = await browser.tabs.query({ windowId: w.id, hidden: true });
        out.push(...hidden);
      } catch (_) {
        // Firefox may not support `hidden` query param on some builds.
      }
    } catch (e) {
      console.warn(`[zenctl] tabs.query windowId=${w.id} failed:`, e?.message ?? e);
    }
  }
  return out;
}

function filterTabs(tabs, params = {}) {
  const target = params.target ?? params;
  return tabs.filter((tab) => {
    if (target.tab_id !== undefined && target.tab_id !== null && tab.id !== target.tab_id) return false;
    if (target.window_id !== undefined && target.window_id !== null && tab.windowId !== target.window_id) return false;
    if (target.active && !tab.active) return false;
    if (target.tab_index !== undefined && target.tab_index !== null && tab.index !== target.tab_index) return false;
    if (target.url_contains && !(tab.url || "").includes(target.url_contains)) return false;
    if (target.title_contains && !(tab.title || "").includes(target.title_contains)) return false;
    return true;
  });
}

async function runPageScript(params, source) {
  const id = await targetTabId(params);
  const [result] = await browser.tabs.executeScript(id, { code: source, allFrames: false });
  return result;
}

function normalizePageRef(params = {}) {
  if (!params.ref) return params;
  const m = String(params.ref).match(/^f(\d+):e\d+$/);
  if (!m) throw new Error("invalid ref: " + params.ref);
  return { ...params, frame_index: Number(m[1]) };
}

async function restoreSessionOfType(kind, sessionId) {
  if (kind !== "tab" && kind !== "window") throw new Error("invalid session kind");
  let id = sessionId;
  if (!id) {
    const closed = await browser.sessions.getRecentlyClosed({ maxResults: 25 });
    const hit = closed.find((entry) => entry && entry[kind]?.sessionId);
    if (!hit) throw new Error(`no recently closed ${kind} found`);
    id = hit[kind].sessionId;
  } else {
    const closed = await browser.sessions.getRecentlyClosed({ maxResults: 100 });
    const hit = closed.find((entry) => entry?.[kind]?.sessionId === id);
    if (!hit) throw new Error(`session_id is not a recently closed ${kind}: ${id}`);
  }
  return await browser.sessions.restore(id);
}

// ---------------------------------------------------------------------------
// Frame-aware script execution via webNavigation.getAllFrames + frameId
// ---------------------------------------------------------------------------

// Get ordered frame list for a tab. Index 0 = main frame.
async function getFrameList(tabId) {
  const frames = await browser.webNavigation.getAllFrames({ tabId });
  // Sort: main frame (parentFrameId === -1) first, then by frameId asc.
  frames.sort((a, b) => {
    if (a.parentFrameId === -1) return -1;
    if (b.parentFrameId === -1) return 1;
    return a.frameId - b.frameId;
  });
  return frames;
}

// Execute script in a specific frame by index. Returns result or null on error.
async function execInFrame(tabId, frameId, source) {
  try {
    const [result] = await browser.tabs.executeScript(tabId, { code: source, frameId });
    return result;
  } catch (e) {
    return null;
  }
}

// Run in all frames (or a specific frame_index). Returns first meaningful result.
// Uses webNavigation.getAllFrames for proper frameId targeting.
async function runPageScriptAllFrames(params, source) {
  const id = await targetTabId(params);
  const frames = await getFrameList(id);

  if (params.frame_index != null) {
    const frame = frames[params.frame_index];
    if (!frame) return null;
    return await execInFrame(id, frame.frameId, source);
  }

  // Try each frame, return first meaningful result.
  for (const frame of frames) {
    const result = await execInFrame(id, frame.frameId, source);
    if (result === null || result === undefined) continue;
    if (typeof result === 'string' && (result === '' || result === '[]')) continue;
    if (Array.isArray(result) && result.length === 0) continue;
    return result;
  }
  return null;
}

// Run in all frames and merge text content from every frame.
async function runPageScriptText(params, source) {
  const id = await targetTabId(params);
  const frames = await getFrameList(id);
  const parts = [];

  if (params.frame_index != null) {
    const frame = frames[params.frame_index];
    if (!frame) return { text: '' };
    const r = await execInFrame(id, frame.frameId, source);
    return r && r.text ? { text: r.text } : { text: '' };
  }

  for (const frame of frames) {
    const r = await execInFrame(id, frame.frameId, source);
    if (r && r.text) parts.push(r.text);
  }
  return { text: parts.join('\n\n--- [frame] ---\n\n') };
}

// Run in all frames and merge interactive element lists.
async function runPageScriptSnapshot(params, source) {
  const id = await targetTabId(params);
  const frames = await getFrameList(id);

  if (params.frame_index != null) {
    const frame = frames[params.frame_index];
    if (!frame) return { elements: [] };
    const r = await execInFrame(id, frame.frameId, source);
    return r && r.elements
      ? { elements: r.elements.map(el => ({ ...el, frameIndex: params.frame_index, ref: `f${params.frame_index}:e${el.index}` })) }
      : { elements: [] };
  }

  const allElements = [];
  for (let i = 0; i < frames.length; i++) {
    const r = await execInFrame(id, frames[i].frameId, source);
    if (r && Array.isArray(r.elements)) {
      for (const el of r.elements) allElements.push({ ...el, frameIndex: i, ref: `f${i}:e${el.index}` });
    }
  }
  return { elements: allElements };
}

function pageScript(name, params = {}) {
  return `(() => {
    const params = ${JSON.stringify(params)};
    const visible = (el) => {
      const r = el.getBoundingClientRect();
      const s = getComputedStyle(el);
      return !!(r.width || r.height) && s.visibility !== "hidden" && s.display !== "none";
    };
    const cssPath = (el) => {
      if (!el || el === document.documentElement) return "html";
      const parts = [];
      for (let n = el; n && n.nodeType === 1 && n !== document; n = n.parentElement) {
        let part = n.localName;
        if (n.id) {
          const idSelector = "#" + CSS.escape(n.id);
          if (document.querySelectorAll(idSelector).length === 1) {
            part = idSelector;
            parts.unshift(part);
            break;
          }
          part += idSelector;
        }
        if (n.classList && n.classList.length) part += "." + [...n.classList].slice(0, 2).map(CSS.escape).join(".");
        const parent = n.parentElement;
        if (parent) {
          const same = [...parent.children].filter(c => c.localName === n.localName);
          if (same.length > 1) part += ":nth-of-type(" + (same.indexOf(n) + 1) + ")";
        }
        parts.unshift(part);
        if (parts.length >= 5) break;
      }
      return parts.join(" > ");
    };
    const pickText = (el) => (el.innerText || el.textContent || el.value || el.getAttribute("aria-label") || "").trim().replace(/\s+/g, " ").slice(0, 160);
    const interactiveSelector = 'a,button,input,textarea,select,[role="button"],[role="link"],[tabindex]:not([tabindex="-1"]),[contenteditable="true"]';
    const find = (selector) => {
      if (params.ref) {
        const m = String(params.ref).match(/^f\\d+:e(\\d+)$/);
        if (!m) throw new Error("invalid ref: " + params.ref);
        const all = [...document.querySelectorAll(interactiveSelector)].filter(visible);
        const el = all[Number(m[1])];
        if (!el) throw new Error("ref not found: " + params.ref);
        return el;
      }
      if (!selector) throw new Error("selector required");
      const nth = params.nth ?? 1; // 1-based index, default first match
      if (nth === 1) {
        const el = document.querySelector(selector);
        if (!el) throw new Error("selector not found: " + selector);
        return el;
      }
      const all = document.querySelectorAll(selector);
      if (all.length < nth) throw new Error("selector matched " + all.length + " elements, wanted nth=" + nth + ": " + selector);
      return all[nth - 1];
    };
    const actions = {
      info() {
        const active = document.activeElement;
        return {
          url: location.href,
          title: document.title,
          ready_state: document.readyState,
          active_element: active ? { tag: active.localName, text: pickText(active), selector: cssPath(active) } : null,
        };
      },
      text() {
        return { url: location.href, title: document.title, text: (document.body?.innerText || document.documentElement?.innerText || "").trim() };
      },
      async source() {
        const html = document.documentElement?.outerHTML || "";
        if (html.length < 900_000) {
          return { url: location.href, title: document.title, html, length: html.length };
        }
        // Large page: compress with gzip then base64 to stay under the
        // 1 MB native-messaging message limit.
        try {
          const encoder = new TextEncoder();
          const data = encoder.encode(html);
          const cs = new CompressionStream("gzip");
          const w = cs.writable.getWriter();
          w.write(data);
          w.close();
          const buf = await new Response(cs.readable).arrayBuffer();
          const bytes = new Uint8Array(buf);
          // Binary → base64 via the charCode / btoa dance
          let b64 = "";
          for (let i = 0; i < bytes.byteLength; i++) b64 += String.fromCharCode(bytes[i]);
          return {
            url: location.href, title: document.title,
            html: btoa(b64),
            compressed: "gzip+base64",
            original_length: html.length,
            compressed_length: bytes.byteLength,
          };
        } catch (e) {
          // Fallback: truncate to ~900 KB so it fits the transport
          return {
            url: location.href, title: document.title,
            html: html.slice(0, 900_000),
            length: Math.min(html.length, 900_000),
            truncated: true,
            compress_error: String(e),
          };
        }
      },
      snapshot() {
        const limit = Math.max(1, Math.min(params.limit || 50, 200));
        const selector = 'a,button,input,textarea,select,[role="button"],[role="link"],[tabindex]:not([tabindex="-1"]),[contenteditable="true"]';
        const elements = [...document.querySelectorAll(selector)].filter(visible).slice(0, limit).map((el, index) => ({
          index,
          tag: el.localName,
          type: el.getAttribute("type") || null,
          role: el.getAttribute("role") || null,
          text: pickText(el),
          aria_label: el.getAttribute("aria-label") || null,
          placeholder: el.getAttribute("placeholder") || null,
          selector: cssPath(el),
        }));
        return { url: location.href, title: document.title, elements };
      },
      click() {
        const el = find(params.selector);
        el.scrollIntoView({ block: "center", inline: "center", behavior: "instant" });
        const rect = el.getBoundingClientRect();
        const cx = rect.left + rect.width / 2;
        const cy = rect.top + rect.height / 2;
        const mOpts = { bubbles: true, cancelable: true, view: window, clientX: cx, clientY: cy };
        const pOpts = { ...mOpts, pointerId: 1, pointerType: "mouse", isPrimary: true };
        // Full pointer + mouse event sequence — required for React 17+ and other
        // frameworks that use pointer-event delegation instead of mouse events.
        el.dispatchEvent(new PointerEvent("pointerover", pOpts));
        el.dispatchEvent(new PointerEvent("pointerenter", { ...pOpts, bubbles: false }));
        el.dispatchEvent(new MouseEvent("mouseover", mOpts));
        el.dispatchEvent(new MouseEvent("mouseenter", { ...mOpts, bubbles: false }));
        el.dispatchEvent(new PointerEvent("pointermove", pOpts));
        el.dispatchEvent(new MouseEvent("mousemove", mOpts));
        el.dispatchEvent(new PointerEvent("pointerdown", { ...pOpts, button: 0, buttons: 1 }));
        el.dispatchEvent(new MouseEvent("mousedown", { ...mOpts, button: 0, buttons: 1 }));
        if (typeof el.focus === "function") el.focus({ preventScroll: true });
        el.dispatchEvent(new PointerEvent("pointerup", { ...pOpts, button: 0, buttons: 0 }));
        el.dispatchEvent(new MouseEvent("mouseup", { ...mOpts, button: 0, buttons: 0 }));
        el.dispatchEvent(new MouseEvent("click", { ...mOpts, button: 0, buttons: 0 }));
        return { clicked: true, selector: params.selector, text: pickText(el) };
      },
      type() {
        const el = find(params.selector);
        const text = String(params.text ?? "");
        el.scrollIntoView({ block: "center", inline: "center", behavior: "instant" });
        el.focus();
        if (el.isContentEditable) el.textContent = text;
        else if ("value" in el) el.value = text;
        else el.textContent = text;
        el.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText", data: text }));
        el.dispatchEvent(new Event("change", { bubbles: true }));
        if (params.submit) {
          el.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", code: "Enter", bubbles: true, cancelable: true }));
          el.dispatchEvent(new KeyboardEvent("keyup", { key: "Enter", code: "Enter", bubbles: true, cancelable: true }));
          const form = el.closest?.("form");
          if (form) form.requestSubmit ? form.requestSubmit() : form.submit();
        }
        return { typed: true, selector: params.selector, submitted: !!params.submit };
      },
      key() {
        const key = String(params.key || "");
        if (!key) throw new Error("key required");
        const target = document.activeElement || document.body || document.documentElement;
        const init = { key, code: key, bubbles: true, cancelable: true };
        target.dispatchEvent(new KeyboardEvent("keydown", init));
        target.dispatchEvent(new KeyboardEvent("keyup", init));
        return { sent: true, key };
      },
      wait() {
        const selector = params.selector;
        const waitText = params.text; // wait for text to appear in page
        const timeout = Number(params.timeout || 5000);
        const nth = params.nth ?? 1;
        const start = Date.now();
        return new Promise((resolve, reject) => {
          const check = () => {
            // Mode 1: wait for text to appear anywhere on page
            if (waitText && !selector) {
              if (document.body && document.body.innerText.includes(waitText)) {
                return resolve({ found: true, text: waitText, elapsed_ms: Date.now() - start });
              }
            }
            // Mode 2: wait for selector (optionally nth match)
            else if (selector) {
              let el;
              if (nth === 1) {
                el = document.querySelector(selector);
              } else {
                const all = document.querySelectorAll(selector);
                el = all.length >= nth ? all[nth - 1] : null;
              }
              if (el && visible(el)) {
                // If waitText also specified, check element contains it
                if (waitText && !pickText(el).includes(waitText)) {
                  // text not matched yet, keep waiting
                } else {
                  return resolve({ found: true, selector, nth, elapsed_ms: Date.now() - start, text: pickText(el) });
                }
              }
            }
            if (Date.now() - start >= timeout) {
              return reject(new Error("timeout waiting for " + (selector || 'text: ' + waitText)));
            }
            setTimeout(check, 100);
          };
          check();
        });
      },
      mediaStatus() {
        const media = [...document.querySelectorAll("video,audio")].find(m => !Number.isNaN(m.duration)) || null;
        return {
          url: location.href,
          title: document.title,
          media: media ? { paused: media.paused, muted: media.muted, current_time: media.currentTime, duration: media.duration, volume: media.volume } : null,
        };
      },
      mediaCommand() {
        const command = params.command;
        const media = [...document.querySelectorAll("video,audio")].find(m => !Number.isNaN(m.duration)) || null;
        if (media && command === "play") media.play();
        else if (media && command === "pause") media.pause();
        else if (media && command === "toggle") media.paused ? media.play() : media.pause();
        else {
          const labels = {
            play: ["Play"],
            pause: ["Pause"],
            toggle: ["Play", "Pause"],
            next: ["Next", "Next song", "Skip"],
            previous: ["Previous", "Previous song"],
          }[command] || [];
          const buttons = [...document.querySelectorAll('button,[role="button"]')];
          const btn = buttons.find(b => labels.some(label => (b.getAttribute("aria-label") || pickText(b)).toLowerCase().includes(label.toLowerCase())));
          if (!btn) throw new Error("media control not found: " + command);
          btn.click();
        }
        return { command, ...actions.mediaStatus() };
      },
      eval() {
        // 1. Try as expression (fast path): "document.title" → works directly.
        // 2. Try as statements with explicit return: "const x = 1; return x" → works.
        // 3. Auto-return last expression: "const x = 1; x + 1" → inserts return.
        const code = params.code;
        try {
          return Function("return (" + code + ")")();
        } catch (e1) {
          if (!(e1 instanceof SyntaxError)) throw e1;
          // Try as-is (may have explicit return)
          try {
            const result = Function(code)();
            if (result !== undefined) return result;
          } catch (e2) {
            if (!(e2 instanceof SyntaxError)) throw e2;
          }
          // Auto-return last expression: find last statement and prepend 'return'
          // Split by newlines first, then try semicolons for single-line code
          let lines = code.trimEnd().split('\\n');
          if (lines.length === 1 && code.includes(';')) {
            // Single line with semicolons: split by last semicolon
            const lastSemi = code.lastIndexOf(';');
            const before = code.substring(0, lastSemi + 1);
            const after = code.substring(lastSemi + 1).trim();
            if (after && !after.startsWith('return ') && !after.startsWith('if') &&
                !after.startsWith('for') && !after.startsWith('while') && !after.startsWith('//')) {
              try {
                return Function(before + ' return ' + after)();
              } catch (e3) { /* fall through */ }
            }
          }
          const lastLine = lines[lines.length - 1].trim();
          // Don't auto-return if last line is a control structure or already returns
          if (!lastLine.startsWith('return ') && !lastLine.startsWith('if') &&
              !lastLine.startsWith('for') && !lastLine.startsWith('while') &&
              !lastLine.startsWith('//') && !lastLine.endsWith('{') && lastLine !== '}') {
            lines[lines.length - 1] = 'return ' + lines[lines.length - 1];
            try {
              return Function(lines.join('\\n'))();
            } catch (e3) { /* fall through */ }
          }
          // Last resort: run without return (side-effect only)
          return Function(code)();
        }
      },
    };
    return actions[${JSON.stringify(name)}]();
  })();`;
}

// ---------------------------------------------------------------------------
// Helpers: enrich tabs with workspace_id from experiment API
// ---------------------------------------------------------------------------

let _workspaceTabCache = null;
let _workspaceTabCacheTime = 0;

async function getWorkspaceTabMap() {
  // Cache for 2s to avoid hammering the experiment API on rapid calls.
  const now = Date.now();
  if (_workspaceTabCache && now - _workspaceTabCacheTime < 2000) {
    return _workspaceTabCache;
  }
  try {
    const r = await browser.zenChrome.getTabWorkspaces();
    _workspaceTabCache = r?.tabs ?? [];
    _workspaceTabCacheTime = now;
  } catch (e) {
    console.warn("[zenctl] getTabWorkspaces failed:", e?.message ?? e);
    _workspaceTabCache = [];
  }
  return _workspaceTabCache;
}

function enrichTabs(tabs, wsTabs) {
  // Build lookup: by tab_id first, fallback by url+window
  const byId = new Map();
  const byUrlWin = new Map();
  for (const wt of wsTabs) {
    if (wt.tab_id != null) {
      byId.set(wt.tab_id, { id: wt.workspace_id, name: wt.workspace_name });
    }
    const key = `${wt.url}|${wt.window_id}`;
    if (!byUrlWin.has(key)) {
      byUrlWin.set(key, { id: wt.workspace_id, name: wt.workspace_name });
    }
  }
  return tabs.map((tab) => {
    const info =
      byId.get(tab.id) ??
      byUrlWin.get(`${tab.url}|${tab.windowId}`) ??
      null;
    return { ...tab, workspace_id: info?.id ?? "", workspace_name: info?.name ?? "" };
  });
}

// ---------------------------------------------------------------------------
// Request handlers (browser API calls)
// ---------------------------------------------------------------------------

const handlers = {
  async bookmarks_list({ folder_id } = {}) {
    if (folder_id) return await browser.bookmarks.getSubTree(folder_id);
    return await browser.bookmarks.getTree();
  },
  async tabs_list({ window_id, current_window, workspace } = {}) {
    let tabs;
    if (window_id !== undefined) {
      tabs = await browser.tabs.query({ windowId: window_id });
    } else if (current_window) {
      tabs = await browser.tabs.query({ currentWindow: true });
    } else {
      tabs = await queryAllTabsSafe();
    }
    if (!workspace) return tabs;
    // Enrich with workspace IDs then filter.
    const wsTabs = await getWorkspaceTabMap();
    const enriched = enrichTabs(tabs, wsTabs);
    const lower = workspace.toLowerCase();
    return enriched.filter((t) => {
      const wid = (t.workspace_id || "").toLowerCase();
      const wname = (t.workspace_name || "").toLowerCase();
      return wid === lower || wid.startsWith(lower) || wname === lower || wname.startsWith(lower);
    });
  },
  async tabs_find(params = {}) {
    const target = params.target ?? params;
    if (target.tab_id !== undefined && target.tab_id !== null) {
      try {
        const t = await browser.tabs.get(target.tab_id);
        return filterTabs([t], params);
      } catch (e) {
        return [];
      }
    }
    let found = filterTabs(await queryAllTabsSafe(), params);
    if (params.workspace) {
      const wsTabs = await getWorkspaceTabMap();
      const enriched = enrichTabs(found, wsTabs);
      const lower = params.workspace.toLowerCase();
      found = enriched.filter((t) => {
        const wid = (t.workspace_id || "").toLowerCase();
        const wname = (t.workspace_name || "").toLowerCase();
        return wid === lower || wid.startsWith(lower) || wname === lower || wname.startsWith(lower);
      });
    }
    return found;
  },
  async tabs_open({ url, active = true, window_id } = {}) {
    if (!url) throw new Error("url required");
    const opts = { url, active };
    if (window_id !== undefined) {
      opts.windowId = window_id;
      return await browser.tabs.create(opts);
    }
    // No focused window in headless/terminal flows -> tabs.create rejects.
    // Try each normal browser window until one accepts. Skip ones whose
    // gBrowser group state is busted (these reject with the generic
    // "An unexpected error occurred").
    let wins;
    try {
      wins = await browser.windows.getAll({ populate: false, windowTypes: ["normal"] });
    } catch (e) {
      throw new Error(`windows.getAll failed: ${e?.message ?? e}`);
    }
    let lastErr;
    for (const w of wins) {
      try {
        return await browser.tabs.create({ ...opts, windowId: w.id });
      } catch (e) {
        lastErr = e;
        console.warn(`[zenctl] tabs.create on windowId=${w.id} failed:`, e?.message ?? e);
      }
    }
    throw lastErr ?? new Error("no usable normal window for tabs.create");
  },
  async tabs_close({ tab_id, tab_ids } = {}) {
    const ids = tab_ids ?? (tab_id !== undefined ? [tab_id] : null);
    if (!ids) throw new Error("tab_id or tab_ids required");
    await browser.tabs.remove(ids);
    return { closed: ids.length };
  },
  async tabs_activate({ tab_id } = {}) {
    if (tab_id === undefined) throw new Error("tab_id required");
    return await browser.tabs.update(tab_id, { active: true });
  },
  async tabs_reload(params = {}) {
    const id = await targetTabId(params);
    await browser.tabs.reload(id, { bypassCache: !!params.bypass_cache });
    return { reloaded: true, tab_id: id };
  },
  async tabs_duplicate(params = {}) {
    const id = await targetTabId(params);
    return await browser.tabs.duplicate(id);
  },
  async tabs_discard(params = {}) {
    const id = await targetTabId(params);
    await browser.tabs.discard(id);
    return { discarded: true, tab_id: id };
  },
  async tabs_set_muted(params = {}) {
    const id = await targetTabId(params);
    const t = await browser.tabs.update(id, { muted: !!params.muted });
    return { tab_id: id, muted: t.mutedInfo ? !!t.mutedInfo.muted : !!params.muted };
  },
  async tabs_set_pinned(params = {}) {
    const id = await targetTabId(params);
    const t = await browser.tabs.update(id, { pinned: !!params.pinned });
    return { tab_id: id, pinned: !!t.pinned };
  },
  async tabs_screenshot(params = {}) {
    const id = await targetTabId(params);
    const format = params.format === "jpeg" ? "jpeg" : "png";
    const captureOpts = { format };
    if (format === "jpeg" && params.quality != null) captureOpts.quality = params.quality;

    if (!params.full_page) {
      const data_url = await browser.tabs.captureTab(id, captureOpts);
      return { tab_id: id, format, data_url };
    }

    // --- Full-page scroll-stitch ---
    const [dims] = await browser.tabs.executeScript(id, {
      code: `({
        scrollHeight: Math.max(document.body.scrollHeight, document.documentElement.scrollHeight),
        viewportWidth:  window.innerWidth,
        viewportHeight: window.innerHeight,
        devicePixelRatio: window.devicePixelRatio || 1,
        scrollX: window.scrollX,
        scrollY: window.scrollY,
      })`,
    });
    const { scrollHeight, viewportWidth, viewportHeight } = dims;
    const dpr = dims.devicePixelRatio;
    const savedX = dims.scrollX;
    const savedY = dims.scrollY;
    const maxScrollY = Math.max(0, scrollHeight - viewportHeight);
    const delay = ms => new Promise(r => setTimeout(r, ms));

    const tiles = [];
    let y = 0;
    while (true) {
      const scrollTo = Math.min(y, maxScrollY);
      await browser.tabs.executeScript(id, { code: `window.scrollTo(0, ${scrollTo})` });
      await delay(150);
      const data_url = await browser.tabs.captureTab(id, captureOpts);
      tiles.push({ data_url, y: scrollTo });
      y += viewportHeight;
      if (scrollTo === maxScrollY || y >= scrollHeight) break;
    }

    // Restore original scroll position
    await browser.tabs.executeScript(id, { code: `window.scrollTo(${savedX}, ${savedY})` });

    // Stitch tiles on a device-pixel canvas in the persistent background page DOM.
    // captureTab returns images at device resolution (CSS px * dpr), so size the
    // canvas in device pixels and draw each tile at its device-pixel y offset.
    const devW = Math.round(viewportWidth  * dpr);
    const devH = Math.round(scrollHeight   * dpr);
    const canvas = document.createElement("canvas");
    canvas.width  = devW;
    canvas.height = devH;
    const ctx = canvas.getContext("2d");
    for (const tile of tiles) {
      const img = await new Promise((resolve, reject) => {
        const i = new Image();
        i.onload = () => resolve(i);
        i.onerror = reject;
        i.src = tile.data_url;
      });
      ctx.drawImage(img, 0, Math.round(tile.y * dpr));
    }

    const stitched = canvas.toDataURL(
      format === "jpeg" ? "image/jpeg" : "image/png",
      format === "jpeg" && params.quality != null ? params.quality / 100 : undefined,
    );
    return { tab_id: id, format, data_url: stitched, full_page: true, width: canvas.width, height: canvas.height };
  },
  async tabs_zoom(params = {}) {
    const id = await targetTabId(params);
    if (params.value != null) {
      await browser.tabs.setZoom(id, params.value);
    }
    const zoom = await browser.tabs.getZoom(id);
    return { tab_id: id, zoom };
  },
  async tabs_reader(params = {}) {
    const id = await targetTabId(params);
    await browser.tabs.toggleReaderMode(id);
    return { tab_id: id, toggled: true };
  },
  async tabs_go_back(params = {}) {
    const id = await targetTabId(params);
    await browser.tabs.goBack(id);
    return { tab_id: id, navigated: "back" };
  },
  async tabs_go_forward(params = {}) {
    const id = await targetTabId(params);
    await browser.tabs.goForward(id);
    return { tab_id: id, navigated: "forward" };
  },
  async windows_list() {
    return await browser.windows.getAll({ populate: false });
  },
  async windows_focus({ window_id } = {}) {
    if (window_id === undefined) throw new Error("window_id required");
    return await browser.windows.update(window_id, { focused: true });
  },
  async windows_close({ window_id } = {}) {
    if (window_id === undefined) throw new Error("window_id required");
    await browser.windows.remove(window_id);
    return { closed: true, window_id };
  },
  async windows_create({ url, incognito, state, type: winType } = {}) {
    const opts = {};
    if (url) opts.url = url;
    if (incognito) opts.incognito = true;
    if (state) opts.state = state;
    if (winType) opts.type = winType;
    return await browser.windows.create(opts);
  },
  async windows_update({ window_id, state, focused } = {}) {
    if (window_id === undefined) throw new Error("window_id required");
    const changes = {};
    if (state !== undefined) changes.state = state;
    if (focused !== undefined) changes.focused = focused;
    if (Object.keys(changes).length === 0) throw new Error("nothing to update");
    return await browser.windows.update(window_id, changes);
  },
  async bookmarks_create({ parent_id, title, url, index } = {}) {
    if (!title) throw new Error("title required");
    const obj = { title };
    if (parent_id !== undefined) obj.parentId = parent_id;
    if (url !== undefined) obj.url = url;
    if (index !== undefined) obj.index = index;
    return await browser.bookmarks.create(obj);
  },
  async bookmarks_update({ id, title, url } = {}) {
    if (!id) throw new Error("id required");
    const changes = {};
    if (title !== undefined) changes.title = title;
    if (url !== undefined) changes.url = url;
    return await browser.bookmarks.update(id, changes);
  },
  async bookmarks_remove({ id, recursive = false } = {}) {
    if (!id) throw new Error("id required");
    if (recursive) await browser.bookmarks.removeTree(id);
    else await browser.bookmarks.remove(id);
    return { removed: id };
  },
  async bookmarks_move({ id, parent_id, index } = {}) {
    if (!id) throw new Error("id required");
    const dest = {};
    if (parent_id !== undefined) dest.parentId = parent_id;
    if (index !== undefined) dest.index = index;
    return await browser.bookmarks.move(id, dest);
  },
  async bookmarks_search({ query } = {}) {
    if (!query) throw new Error("query required");
    return await browser.bookmarks.search(query);
  },
  async tabs_move({ tab_id, tab_ids, window_id, index } = {}) {
    const ids = tab_ids ?? (tab_id !== undefined ? [tab_id] : null);
    if (!ids) throw new Error("tab_id or tab_ids required");
    if (index === undefined) throw new Error("index required");
    const props = { index };
    if (window_id !== undefined) props.windowId = window_id;
    return await browser.tabs.move(ids, props);
  },
  async history_search({ query = "", max_results = 50, start_time } = {}) {
    const params = { text: query, maxResults: max_results };
    if (start_time !== undefined) params.startTime = start_time;
    return await browser.history.search(params);
  },
  async history_delete({ url } = {}) {
    if (!url) throw new Error("url required");
    await browser.history.deleteUrl({ url });
    return { deleted: url };
  },
  async history_add({ url, title } = {}) {
    if (!url) throw new Error("url required");
    const details = { url };
    if (title !== undefined) details.title = title;
    await browser.history.addUrl(details);
    return { added: url };
  },
  async history_get_visits({ url } = {}) {
    if (!url) throw new Error("url required");
    return await browser.history.getVisits({ url });
  },
  async downloads_list({ query = "" } = {}) {
    return await browser.downloads.search({ query: query ? [query] : [] });
  },
  async downloads_cancel({ download_id } = {}) {
    if (download_id === undefined) throw new Error("download_id required");
    await browser.downloads.cancel(download_id);
    return { cancelled: download_id };
  },
  async downloads_start({ url, filename, save_as } = {}) {
    if (!url) throw new Error("url required");
    const details = { url };
    if (filename) details.filename = filename;
    if (save_as !== undefined) details.saveAs = !!save_as;
    const id = await browser.downloads.download(details);
    return { download_id: id, url };
  },
  async downloads_pause({ download_id } = {}) {
    if (download_id === undefined) throw new Error("download_id required");
    await browser.downloads.pause(download_id);
    return { paused: download_id };
  },
  async downloads_resume({ download_id } = {}) {
    if (download_id === undefined) throw new Error("download_id required");
    await browser.downloads.resume(download_id);
    return { resumed: download_id };
  },
  async cookies_get({ url, name } = {}) {
    if (!url || !name) throw new Error("url + name required");
    return await browser.cookies.get({ url, name });
  },
  async cookies_set({ url, name, value, domain, path: cookiePath, secure, http_only, expiry } = {}) {
    if (!url || !name || value === undefined) throw new Error("url, name, value required");
    const details = { url, name, value };
    if (domain !== undefined) details.domain = domain;
    if (cookiePath !== undefined) details.path = cookiePath;
    if (secure !== undefined) details.secure = secure;
    if (http_only !== undefined) details.httpOnly = http_only;
    if (expiry !== undefined) details.expirationDate = expiry;
    return await browser.cookies.set(details);
  },
  async cookies_remove({ url, name } = {}) {
    if (!url || !name) throw new Error("url + name required");
    return await browser.cookies.remove({ url, name });
  },

  async tab_group({ tab_ids, group_id, create_properties = {} } = {}) {
    if (!Array.isArray(tab_ids) || tab_ids.length === 0) throw new Error("tab_ids required");
    if (!browser.tabs.group) throw new Error("browser.tabs.group unavailable in this Firefox/Zen build");
    const options = { tabIds: tab_ids };
    if (group_id !== undefined && group_id !== null) options.groupId = group_id;
    if (create_properties && Object.keys(create_properties).length) options.createProperties = create_properties;
    const id = await browser.tabs.group(options);
    return { group_id: id, tab_ids };
  },
  async tab_ungroup({ tab_ids } = {}) {
    if (!Array.isArray(tab_ids) || tab_ids.length === 0) throw new Error("tab_ids required");
    if (!browser.tabs.ungroup) throw new Error("browser.tabs.ungroup unavailable in this Firefox/Zen build");
    await browser.tabs.ungroup(tab_ids);
    return { ungrouped: tab_ids };
  },

  // --- Sessions / browsing-data tier ---
  async sessions_closed({ max_results } = {}) {
    const filter = max_results != null ? { maxResults: max_results } : {};
    return await browser.sessions.getRecentlyClosed(filter);
  },
  async sessions_restore({ session_id } = {}) {
    return await browser.sessions.restore(session_id ?? undefined);
  },
  async session_restore_window({ session_id } = {}) {
    return await restoreSessionOfType("window", session_id);
  },
  async session_restore_tab({ session_id } = {}) {
    return await restoreSessionOfType("tab", session_id);
  },
  async data_clear({ since = 0, types = {} } = {}) {
    const allowed = [
      "cache", "cookies", "history", "downloads", "formData",
      "localStorage", "indexedDB", "pluginData", "passwords", "serviceWorkers",
    ];
    const dataTypes = {};
    for (const k of allowed) {
      if (types[k]) dataTypes[k] = true;
    }
    if (Object.keys(dataTypes).length === 0) {
      throw new Error("no data types selected");
    }
    await browser.browsingData.remove({ since }, dataTypes);
    return { cleared: Object.keys(dataTypes), since };
  },

  // --- Containers (contextual identities) ---
  async containers_list() {
    return await browser.contextualIdentities.query({});
  },
  async containers_create({ name, color = "blue", icon = "circle" } = {}) {
    if (!name) throw new Error("name required");
    return await browser.contextualIdentities.create({ name, color, icon });
  },
  async containers_update({ cookie_store_id, name, color, icon } = {}) {
    if (!cookie_store_id) throw new Error("cookie_store_id required");
    const changes = {};
    if (name !== undefined) changes.name = name;
    if (color !== undefined) changes.color = color;
    if (icon !== undefined) changes.icon = icon;
    return await browser.contextualIdentities.update(cookie_store_id, changes);
  },
  async containers_remove({ cookie_store_id } = {}) {
    if (!cookie_store_id) throw new Error("cookie_store_id required");
    const identities = await browser.contextualIdentities.query({});
    if (!identities.find(c => c.cookieStoreId === cookie_store_id)) {
      throw new Error("container not found: " + cookie_store_id);
    }
    return await browser.contextualIdentities.remove(cookie_store_id);
  },

  // --- Find in page ---
  async find_in_page(params = {}) {
    const query = params.query;
    if (!query) throw new Error("query required");
    const id = await targetTabId(params);
    const opts = { tabId: id };
    if (params.case_sensitive) opts.caseSensitive = true;
    if (params.entire_word) opts.entireWord = true;
    const result = await browser.find.find(query, opts);
    if (result.count > 0) browser.find.highlightResults({ tabId: id });
    return { tab_id: id, query, count: result.count };
  },
  async find_clear() {
    browser.find.removeHighlighting();
    return { cleared: true };
  },

  // --- Search engines ---
  async search_list() {
    return await browser.search.get();
  },
  async search_query(params = {}) {
    const query = params.query;
    if (!query) throw new Error("query required");
    const opts = { query };
    if (params.engine) opts.engine = params.engine;
    try {
      opts.tabId = await targetTabId(params);
    } catch (e) {
      // No resolvable tab — search opens its own.
    }
    await browser.search.search(opts);
    return { query, engine: params.engine ?? null };
  },

  // --- Page interaction tier (via tabs.executeScript) ---
  async page_frames(params = {}) {
    const id = await targetTabId(params);
    const frames = await getFrameList(id);
    return {
      frames: frames.map((f, i) => ({
        index: i,
        frameId: f.frameId,
        parentFrameId: f.parentFrameId,
        url: f.url,
      })),
    };
  },
  async page_info(params = {}) {
    return await runPageScript(params, pageScript("info"));
  },
  async page_text(params = {}) {
    // Combine text from all frames (incl. cross-origin iframes like Power Apps).
    return await runPageScriptText(params, pageScript("text"));
  },
  async page_source(params = {}) {
    return await runPageScript(params, pageScript("source"));
  },
  async page_snapshot(params = {}) {
    // Merge interactive elements from all frames.
    return await runPageScriptSnapshot(params, pageScript("snapshot", { limit: params.limit ?? 50 }));
  },
  async page_click(params = {}) {
    params = normalizePageRef(params);
    return await runPageScriptAllFrames(params, pageScript("click", { selector: params.selector, nth: params.nth, ref: params.ref }));
  },
  async page_type(params = {}) {
    params = normalizePageRef(params);
    return await runPageScriptAllFrames(params, pageScript("type", { selector: params.selector, text: params.text, submit: !!params.submit, nth: params.nth, ref: params.ref }));
  },
  async page_key(params = {}) {
    return await runPageScriptAllFrames(params, pageScript("key", { key: params.key }));
  },
  async page_wait(params = {}) {
    return await runPageScriptAllFrames(params, pageScript("wait", { selector: params.selector, timeout: params.timeout ?? 5000, nth: params.nth, text: params.wait_text }));
  },
  async page_eval(params = {}) {
    if (!params.code) throw new Error("code required");
    return await runPageScriptAllFrames(params, pageScript("eval", { code: params.code }));
  },
  async media_status(params = {}) {
    return await runPageScript(params, pageScript("mediaStatus"));
  },
  async media_play(params = {}) {
    return await runPageScript(params, pageScript("mediaCommand", { command: "play" }));
  },
  async media_pause(params = {}) {
    return await runPageScript(params, pageScript("mediaCommand", { command: "pause" }));
  },
  async media_toggle(params = {}) {
    return await runPageScript(params, pageScript("mediaCommand", { command: "toggle" }));
  },
  async media_next(params = {}) {
    return await runPageScript(params, pageScript("mediaCommand", { command: "next" }));
  },
  async media_previous(params = {}) {
    return await runPageScript(params, pageScript("mediaCommand", { command: "previous" }));
  },

  // Meta: reload the extension. The native messaging port closes as the
  // extension restarts; the host process exits (stdin closes). The fresh
  // extension page will spawn a new host on reconnect.
  async ext_reload() {
    setTimeout(() => {
      try { browser.runtime.reload(); }
      catch (e) { console.error("[zenctl] reload failed", e); }
    }, 50);
    return { reloading: true };
  },

  // Toggle verbose request/response logging. Persists in extension storage;
  // an onChanged listener at top of file updates the live DEBUG flag.
  // params.enabled absent → status query; bool → set.
  async ext_debug({ enabled } = {}) {
    if (enabled === undefined) {
      return { enabled: DEBUG };
    }
    const next = !!enabled;
    await browser.storage.local.set({ [DEBUG_KEY]: next });
    return { enabled: next };
  },

  // --- Preference tier (via WebExtension Experiments) ---
  async prefs_get({ name } = {}) {
    if (!name) throw new Error("name required");
    requirePrefsApi();
    return await browser.zenPrefs.getPref(name);
  },
  async prefs_set({ name, value } = {}) {
    if (name === undefined || value === undefined) throw new Error("name + value required");
    requirePrefsApi();
    return await browser.zenPrefs.setPref(name, value);
  },
  async prefs_clear({ name } = {}) {
    if (!name) throw new Error("name required");
    requirePrefsApi();
    const clear = browser.zenPrefs.clearUserPref ?? browser.zenPrefs.clearPref;
    if (!clear) throw new Error("browser.zenPrefs.clearUserPref missing");
    return await clear(name);
  },
  async prefs_list({ prefix = "zen." } = {}) {
    requirePrefsApi();
    return await browser.zenPrefs.listPrefs(prefix);
  },

  // --- UI-automation tier (via zenChrome experiment) ---
  async compact_toggle() {
    requireChromeApi();
    const r = await browser.zenChrome.compactToggle();
    return { triggered: true, enabled: !!r.enabled };
  },
  async compact_set({ value } = {}) {
    if (typeof value !== "boolean") throw new Error("value: bool required");
    requireChromeApi();
    const r = await browser.zenChrome.compactSet(value);
    return { triggered: true, enabled: !!r.enabled };
  },
  async workspace_switch({ uuid } = {}) {
    if (!uuid) throw new Error("uuid required");
    requireChromeApi();
    return await browser.zenChrome.workspaceSwitch(uuid);
  },
  async workspace_list() {
    requireChromeApi();
    const live = await browser.zenChrome.workspacesList();
    requirePrefsApi();
    const prefs = await browser.zenPrefs.listPrefs("zen.workspaces.");
    return { active: live.active, workspaces: live.workspaces, prefs };
  },
  async workspace_unload({ uuid } = {}) {
    requireChromeApi();
    return await browser.zenChrome.workspaceUnload(uuid ?? "", false);
  },
  async workspace_unload_all({ except_uuid } = {}) {
    requireChromeApi();
    return await browser.zenChrome.workspaceUnload(except_uuid ?? "", true);
  },
  async glance_close({ tab_id } = {}) {
    requireChromeApi();
    if (tab_id != null) await browser.tabs.update(tab_id, { active: true });
    return await browser.zenChrome.glanceClose();
  },
  async glance_expand({ tab_id } = {}) {
    requireChromeApi();
    if (tab_id != null) await browser.tabs.update(tab_id, { active: true });
    return await browser.zenChrome.glanceExpand();
  },
  async glance_list() {
    requireChromeApi();
    return await browser.zenChrome.glanceList();
  },
  async glance_close_all() {
    requireChromeApi();
    return await browser.zenChrome.glanceCloseAll();
  },
  async split_view_create({ grid_type = "grid", tab_ids } = {}) {
    requireChromeApi();
    const urls = [];
    if (Array.isArray(tab_ids) && tab_ids.length > 0) {
      for (const id of tab_ids) {
        const t = await browser.tabs.get(id);
        urls.push(t.url);
      }
    }
    return await browser.zenChrome.splitViewCreate(JSON.stringify(urls), grid_type);
  },
  async split_unsplit() {
    requireChromeApi();
    return await browser.zenChrome.splitUnsplit();
  },
  async split_view_list() {
    requireChromeApi();
    return await browser.zenChrome.splitViewList();
  },
  async split_view_add_tab({ tab_ids, urls, grid_type } = {}) {
    requireChromeApi();
    const resolved = await resolveTabUrls({ tab_ids, urls });
    if (resolved.length === 0) throw new Error("provide tab_ids[] or urls[]");
    return await browser.zenChrome.splitViewAddTab(JSON.stringify(resolved), grid_type ?? "");
  },
  async split_view_set_layout({ grid_type } = {}) {
    requireChromeApi();
    return await browser.zenChrome.splitViewSetLayout(grid_type ?? "grid");
  },
  async split_view_resize({ path, sizes } = {}) {
    requireChromeApi();
    if (!Array.isArray(sizes) || sizes.length === 0) throw new Error("sizes[] required");
    return await browser.zenChrome.splitViewResize(path ?? "", JSON.stringify(sizes));
  },
  async split_view_rearrange({ enable } = {}) {
    requireChromeApi();
    return await browser.zenChrome.splitViewRearrange(enable !== false);
  },
  async compact_hide({ what } = {}) {
    if (!what) throw new Error("what required (sidebar|toolbar|both)");
    requireChromeApi();
    return await browser.zenChrome.compactHide(what);
  },
  async glance_open({ url } = {}) {
    if (!url) throw new Error("url required");
    requireChromeApi();
    return await browser.zenChrome.glanceOpen(url);
  },
  async urlbar_search({ query, submit } = {}) {
    if (query === undefined || query === null) throw new Error("query required");
    requireChromeApi();
    return await browser.zenChrome.urlbarSearch(query, !!submit);
  },
  async urlbar_close() {
    requireChromeApi();
    return await browser.zenChrome.urlbarClose();
  },
  async urlbar_actions_list() {
    requireChromeApi();
    return await browser.zenChrome.urlbarActionsList();
  },
  async urlbar_actions_run({ action } = {}) {
    if (!action) throw new Error("action required");
    requireChromeApi();
    return await browser.zenChrome.urlbarActionsRun(action);
  },
  async share_can() {
    requireChromeApi();
    return await browser.zenChrome.shareCan();
  },
  async share({ url, title, text } = {}) {
    requireChromeApi();
    if (!url || !title) {
      const tabs = await browser.tabs.query({ active: true, currentWindow: true });
      const tab = tabs[0];
      if (!tab) throw new Error("no active tab to share");
      if (!url) url = tab.url;
      if (!title) title = tab.title ?? "";
    }
    return await browser.zenChrome.share(url, title ?? "", text ?? "");
  },
  async workspace_create({ name, icon } = {}) {
    requireChromeApi();
    return await browser.zenChrome.workspaceCreate(name ?? "Space", icon ?? "");
  },
  async workspace_remove({ uuid } = {}) {
    if (!uuid) throw new Error("uuid required");
    requireChromeApi();
    return await browser.zenChrome.workspaceRemove(uuid);
  },
  async workspace_rename({ uuid, name } = {}) {
    if (!uuid || name === undefined) throw new Error("uuid + name required");
    requireChromeApi();
    return await browser.zenChrome.workspaceRename(uuid, name);
  },
  async workspace_set_icon({ uuid, icon } = {}) {
    if (!uuid || icon === undefined) throw new Error("uuid + icon required");
    requireChromeApi();
    return await browser.zenChrome.workspaceSetIcon(uuid, icon);
  },
  async workspace_set_container({ uuid, cookie_store_id } = {}) {
    if (!uuid || cookie_store_id === undefined) throw new Error("uuid + cookie_store_id required");
    requireChromeApi();
    return await browser.zenChrome.workspaceSetContainer(uuid, cookie_store_id);
  },
  async workspace_reorder({ uuid, index } = {}) {
    if (!uuid || index === undefined) throw new Error("uuid + index required");
    requireChromeApi();
    return await browser.zenChrome.workspaceReorder(uuid, index);
  },
  async workspace_move_tab({ uuid, tab_ids, urls } = {}) {
    if (!uuid) throw new Error("uuid required");
    requireChromeApi();
    const resolved = [];
    if (Array.isArray(urls)) resolved.push(...urls);
    if (Array.isArray(tab_ids)) {
      for (const id of tab_ids) {
        const t = await browser.tabs.get(id);
        if (t?.url) resolved.push(t.url);
      }
    }
    if (resolved.length === 0) throw new Error("provide tab_ids[] or urls[]");
    return await browser.zenChrome.workspaceMoveTab(uuid, JSON.stringify(resolved));
  },

  // --- Zen essentials ---
  async essentials_list() {
    requireChromeApi();
    return await browser.zenChrome.essentialsList();
  },
  async essentials_add({ tab_ids, urls } = {}) {
    requireChromeApi();
    const resolved = await resolveTabUrls({ tab_ids, urls });
    if (resolved.length === 0) throw new Error("provide tab_ids[] or urls[]");
    return await browser.zenChrome.essentialsAdd(JSON.stringify(resolved));
  },
  async essentials_remove({ tab_ids, urls, unpin } = {}) {
    requireChromeApi();
    const resolved = await resolveTabUrls({ tab_ids, urls });
    if (resolved.length === 0) throw new Error("provide tab_ids[] or urls[]");
    return await browser.zenChrome.essentialsRemove(
      JSON.stringify(resolved),
      unpin !== false
    );
  },
  async essentials_reset({ tab_ids, urls } = {}) {
    requireChromeApi();
    const resolved = await resolveTabUrls({ tab_ids, urls });
    if (resolved.length === 0) throw new Error("provide tab_ids[] or urls[]");
    return await browser.zenChrome.essentialsReset(JSON.stringify(resolved));
  },
  async essentials_replace_url({ tab_ids, urls } = {}) {
    requireChromeApi();
    const resolved = await resolveTabUrls({ tab_ids, urls });
    if (resolved.length === 0) throw new Error("provide tab_ids[] or urls[]");
    return await browser.zenChrome.essentialsReplaceUrl(JSON.stringify(resolved));
  },

  // --- Zen mods ---
  async mods_list() {
    requireChromeApi();
    return await browser.zenChrome.modsList();
  },
  async mods_install({ mod_id, url } = {}) {
    if (!mod_id && !url) throw new Error("mod_id or url required");
    requireChromeApi();
    return await browser.zenChrome.modsInstall(mod_id ?? "", url ?? "");
  },
  async mods_remove({ mod_id } = {}) {
    if (!mod_id) throw new Error("mod_id required");
    requireChromeApi();
    return await browser.zenChrome.modsRemove(mod_id);
  },
  async mods_enable({ mod_id } = {}) {
    if (!mod_id) throw new Error("mod_id required");
    requireChromeApi();
    return await browser.zenChrome.modsEnable(mod_id);
  },
  async mods_disable({ mod_id } = {}) {
    if (!mod_id) throw new Error("mod_id required");
    requireChromeApi();
    return await browser.zenChrome.modsDisable(mod_id);
  },
  async mods_preferences({ mod_id } = {}) {
    if (!mod_id) throw new Error("mod_id required");
    requireChromeApi();
    return await browser.zenChrome.modsPreferences(mod_id);
  },
  async mods_set_preference({ mod_id, pref_name, pref_value } = {}) {
    if (!mod_id) throw new Error("mod_id required");
    if (!pref_name) throw new Error("pref_name required");
    if (pref_value === undefined) throw new Error("pref_value required");
    requireChromeApi();
    requirePrefsApi();
    const prefs = await browser.zenChrome.modsPreferences(mod_id);
    const entry = prefs?.preferences?.find(p => p.property === pref_name);
    if (!entry) throw new Error(`preference "${pref_name}" not found on mod "${mod_id}"`);
    const isBool = entry.type === "checkbox";
    if (isBool && typeof pref_value !== "boolean") {
      throw new Error(`preference "${pref_name}" expects boolean, got ${typeof pref_value}`);
    }
    if (!isBool && typeof pref_value !== "string") {
      throw new Error(`preference "${pref_name}" expects string, got ${typeof pref_value}`);
    }
    return await browser.zenPrefs.setPref(pref_name, pref_value);
  },

  async folders_list() {
    requireChromeApi();
    return await browser.zenChrome.foldersList();
  },
  async folders_create({ label, workspace_id } = {}) {
    requireChromeApi();
    return await browser.zenChrome.foldersCreate(label ?? "", workspace_id ?? "");
  },
  async folders_delete({ folder_id } = {}) {
    if (!folder_id) throw new Error("folder_id required");
    requireChromeApi();
    return await browser.zenChrome.foldersDelete(folder_id);
  },
  async folders_rename({ folder_id, name } = {}) {
    if (!folder_id) throw new Error("folder_id required");
    if (!name) throw new Error("name required");
    requireChromeApi();
    return await browser.zenChrome.foldersRename(folder_id, name);
  },
  async folders_collapse({ folder_id, collapsed } = {}) {
    if (!folder_id) throw new Error("folder_id required");
    requireChromeApi();
    return await browser.zenChrome.foldersCollapse(folder_id, !!collapsed);
  },
  async folders_add_tab({ folder_id, tab_ids, urls } = {}) {
    if (!folder_id) throw new Error("folder_id required");
    requireChromeApi();
    const resolved = [];
    if (Array.isArray(urls)) resolved.push(...urls);
    if (Array.isArray(tab_ids)) {
      for (const id of tab_ids) {
        const t = await browser.tabs.get(id);
        if (t?.url) resolved.push(t.url);
      }
    }
    if (resolved.length === 0) throw new Error("provide tab_ids[] or urls[]");
    return await browser.zenChrome.foldersAddTab(folder_id, JSON.stringify(resolved));
  },
  async folders_set_icon({ folder_id, icon } = {}) {
    if (!folder_id) throw new Error("folder_id required");
    requireChromeApi();
    return await browser.zenChrome.foldersSetIcon(folder_id, icon ?? "");
  },
  async folders_create_subfolder({ parent_id, label } = {}) {
    if (!parent_id) throw new Error("parent_id required");
    requireChromeApi();
    return await browser.zenChrome.foldersCreateSubfolder(parent_id, label ?? "");
  },
  async folders_unpack({ folder_id } = {}) {
    if (!folder_id) throw new Error("folder_id required");
    requireChromeApi();
    return await browser.zenChrome.foldersUnpack(folder_id);
  },
  async folders_unload({ folder_id } = {}) {
    if (!folder_id) throw new Error("folder_id required");
    requireChromeApi();
    return await browser.zenChrome.foldersUnload(folder_id);
  },
  async folders_move_to_workspace({ folder_id, workspace_id } = {}) {
    if (!folder_id) throw new Error("folder_id required");
    if (!workspace_id) throw new Error("workspace_id required");
    requireChromeApi();
    return await browser.zenChrome.foldersMoveToWorkspace(folder_id, workspace_id);
  },
  async folders_convert_to_workspace({ folder_id } = {}) {
    if (!folder_id) throw new Error("folder_id required");
    requireChromeApi();
    return await browser.zenChrome.foldersConvertToWorkspace(folder_id);
  },

  async live_folders_list() {
    requireChromeApi();
    return await browser.zenChrome.liveFoldersList();
  },
  async live_folders_create({ provider, url, label } = {}) {
    if (!provider) throw new Error("provider required");
    requireChromeApi();
    return await browser.zenChrome.liveFoldersCreate(provider, url ?? "", label ?? "");
  },
  async live_folders_delete({ folder_id } = {}) {
    if (!folder_id) throw new Error("folder_id required");
    requireChromeApi();
    return await browser.zenChrome.liveFoldersDelete(folder_id);
  },
  async live_folders_refresh({ folder_id } = {}) {
    if (!folder_id) throw new Error("folder_id required");
    requireChromeApi();
    return await browser.zenChrome.liveFoldersRefresh(folder_id);
  },
  async live_folders_pause({ folder_id } = {}) {
    if (!folder_id) throw new Error("folder_id required");
    requireChromeApi();
    return await browser.zenChrome.liveFoldersPause(folder_id);
  },
  async live_folders_resume({ folder_id } = {}) {
    if (!folder_id) throw new Error("folder_id required");
    requireChromeApi();
    return await browser.zenChrome.liveFoldersResume(folder_id);
  },

  async window_sync_force() {
    requireChromeApi();
    return await browser.zenChrome.windowSyncForce();
  },

  async shortcuts_reset() {
    requireChromeApi();
    return await browser.zenChrome.shortcutsReset();
  },

  async boosts_list() {
    requireChromeApi();
    return await browser.zenChrome.boostsList();
  },
  async boosts_create({ domain } = {}) {
    if (!domain) throw new Error("domain required");
    requireChromeApi();
    return await browser.zenChrome.boostsCreate(domain);
  },
  async boosts_delete({ domain, id } = {}) {
    if (!domain || !id) throw new Error("domain + id required");
    requireChromeApi();
    return await browser.zenChrome.boostsDelete(domain, id);
  },
  async boosts_activate({ domain, id } = {}) {
    if (!domain || !id) throw new Error("domain + id required");
    requireChromeApi();
    return await browser.zenChrome.boostsActivate(domain, id);
  },
  async boosts_toggle({ domain, id } = {}) {
    if (!domain || !id) throw new Error("domain + id required");
    requireChromeApi();
    return await browser.zenChrome.boostsToggle(domain, id);
  },
  async boosts_update({ domain, id, data_json } = {}) {
    if (!domain || !id) throw new Error("domain + id required");
    if (!data_json) throw new Error("data_json required");
    requireChromeApi();
    return await browser.zenChrome.boostsUpdate(domain, id, data_json);
  },

  // TabDetach: detach a tab into its own window via standard WebExtension APIs.
  async tab_detach({ tab_id = null } = {}) {
    if (tab_id == null) throw new Error("tab_id required");
    const tab = await browser.tabs.get(tab_id);
    if (!tab) throw new Error(`tab ${tab_id} not found`);
    const win = await browser.windows.create({ url: tab.url, incognito: tab.incognito });
    // Only remove the old tab if a new window was actually created.
    if (win?.id) {
      await browser.tabs.remove(tab_id);
      const [newTab] = await browser.tabs.query({ windowId: win.id });
      return { tab_id: newTab?.id ?? null, window_id: win.id, url: tab.url };
    }
    return { error: "failed to create window" };
  },

  // Internal: report live experiment-API availability for `zenctl capabilities`.
  async capabilities_probe() {
    const result = {
      zen_chrome: !!(browser.zenChrome && browser.zenChrome.compactToggle),
      zen_prefs: !!(browser.zenPrefs && browser.zenPrefs.getPref),
    };
    console.log(
      `[zenctl] probe: zenChrome=${result.zen_chrome ? "ok" : "missing"} zenPrefs=${result.zen_prefs ? "ok" : "missing"}`
    );
    return result;
  },
};

function requireChromeApi() {
  if (!browser.zenChrome?.compactToggle) {
    throw new Error(
      "browser.zenChrome missing — enable extensions.experiments.enabled in about:config"
    );
  }
}

// Collect tab URLs from explicit `urls[]` plus any `tab_ids[]` resolved via
// browser.tabs.get. Mirrors the inline resolution in workspace_move_tab.
async function resolveTabUrls({ tab_ids, urls } = {}) {
  const resolved = [];
  if (Array.isArray(urls)) resolved.push(...urls);
  if (Array.isArray(tab_ids)) {
    for (const id of tab_ids) {
      const t = await browser.tabs.get(id);
      if (t?.url) resolved.push(t.url);
    }
  }
  return resolved;
}

function requirePrefsApi() {
  if (!browser.zenPrefs?.getPref) {
    throw new Error(
      "browser.zenPrefs missing — enable extensions.experiments.enabled in about:config"
    );
  }
}

async function handleRequest(req) {
  const handler = handlers[req.method];
  if (!handler) {
    // Always-on: a typo or stale CLI/extension version is the #1 cause of
    // silent "nothing happens" — surface it instead of returning quietly.
    console.warn(`[zenctl] no handler for "${req.method}" (id=${req.id})`);
    sendError(req.id, "unsupported");
    return;
  }
  const t0 = DEBUG ? performance.now() : 0;
  if (DEBUG) console.log(`[zenctl] ← ${req.method}#${req.id}`);
  try {
    const result = await handler(req.params ?? {});
    if (DEBUG) {
      const ms = (performance.now() - t0).toFixed(1);
      console.log(`[zenctl] → ${req.method}#${req.id} ok ${ms}ms`);
    }
    sendOk(req.id, result);
  } catch (err) {
    console.error(`[zenctl] ${req.method} failed:`, err);
    const parts = [];
    if (err?.message) parts.push(err.message);
    if (err?.fileName) parts.push(`@${err.fileName}:${err.lineNumber ?? "?"}`);
    if (err?.stack) parts.push(`\n${String(err.stack).split("\n").slice(0, 3).join(" | ")}`);
    const detail = parts.length ? parts.join(" ") : String(err);
    sendError(req.id, "internal", detail);
  }
}

// ---------------------------------------------------------------------------
// Browser event listeners — forwarded to `zenctl watch` clients as events
// ---------------------------------------------------------------------------

function registerEventListeners() {
  const t = browser.tabs;
  if (t.onCreated) {
    t.onCreated.addListener((tab) =>
      emitEvent("tabs.created", { tab_id: tab.id, window_id: tab.windowId, url: tab.url }));
  }
  if (t.onRemoved) {
    t.onRemoved.addListener((tabId, info) =>
      emitEvent("tabs.removed", { tab_id: tabId, window_id: info.windowId }));
  }
  if (t.onActivated) {
    t.onActivated.addListener((info) =>
      emitEvent("tabs.activated", { tab_id: info.tabId, window_id: info.windowId }));
  }
  if (t.onUpdated) {
    t.onUpdated.addListener((tabId, change, tab) =>
      emitEvent("tabs.updated", { tab_id: tabId, change, url: tab.url }));
  }
  const w = browser.windows;
  if (w.onCreated) {
    w.onCreated.addListener((win) => emitEvent("windows.created", { window_id: win.id }));
  }
  if (w.onRemoved) {
    w.onRemoved.addListener((winId) => emitEvent("windows.removed", { window_id: winId }));
  }
  if (w.onFocusChanged) {
    w.onFocusChanged.addListener((winId) =>
      emitEvent("windows.focus_changed", { window_id: winId }));
  }
}

// ---------------------------------------------------------------------------
// Start
// ---------------------------------------------------------------------------

registerEventListeners();
connect();
