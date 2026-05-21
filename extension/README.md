# zenctl Extension

Two manifest variants are tracked here:

- `manifest-basic.json` — standard features (bookmarks, tabs, windows,
  history, downloads, cookies, page/media scripting).
- `manifest-privileged.json` — adds `experiment_apis` for preferences,
  compact mode, workspaces, glance, split view, shortcuts, and session
  read/backup. Needs `extensions.experiments.enabled = true`.

`manifest.json` is git-ignored. `about:debugging` only loads a file named
`manifest.json`, so `zenctl ext use` symlinks the chosen variant into place.

## Quick start (basic features)

```bash
zenctl ext use basic
```

1. Open `about:debugging#/runtime/this-firefox` in Zen Browser
2. Click **Load Temporary Add-on**
3. Select `extension/manifest.json`

## Privileged features (prefs / chrome ops / shortcuts / glance / split / session)

```bash
zenctl ext use privileged
```

1. Set `extensions.experiments.enabled = true` in `about:config`
2. Open `about:debugging#/runtime/this-firefox`
3. Click **Load Temporary Add-on**
4. Select `extension/manifest.json`

### Autoconfig / startup install

1. Run `zenctl ext use privileged`
2. Create `<zen-profile>/autoconfig.js`:
   ```js
   pref("extensions.experiments.enabled", true);
   ```
3. Add `<zen-install-dir>/defaults/pref/autoconfig.js`:
   ```js
   pref("general.config.filename", "autoconfig.cfg");
   pref("general.config.obscure_value", 0);
   ```
4. **Restart Zen completely**.

### Enterprise policies

Create `<zen-install-dir>/distribution/policies.json`:
```json
{
  "policies": {
    "Extensions": {
      "Install": ["<path>/extension/manifest-privileged.json"]
    }
  }
}
```

### When privileged features are unavailable

The background script returns clear errors:
- `browser.zenChrome missing — enable extensions.experiments.enabled in about:config`
- `browser.zenPrefs missing — enable extensions.experiments.enabled in about:config`

All standard features continue working normally.

## Page and media control

`zenctl page ...` and `zenctl media ...` can target a specific tab/window:

```bash
zenctl page info --url-contains music.youtube
zenctl page snapshot --window-id 30 --limit 20
zenctl page click 'button[aria-label="Play"]' --title-contains 'YouTube Music'
zenctl page key Escape --tab-id 19
zenctl page wait 'body' --url-contains music.youtube
zenctl media status --url-contains music.youtube
zenctl media toggle --url-contains music.youtube
```

Global host selection hooks are also available for future multi-profile hosts:

```bash
zenctl --socket /tmp/zenctl-work.sock status
zenctl --profile work status
```
