//! `zenctl install` — registers the native messaging host manifest so the
//! extension can spawn this binary directly without any user configuration.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use zenctl_protocol::{Capability, Method, Tier};

/// The Firefox extension ID declared in extension/manifest.json.
pub const EXTENSION_ID: &str = "zenctl@phusit.local";
/// The native messaging host name — must match the string passed to
/// `browser.runtime.connectNative()` in background.js.
pub const HOST_NAME: &str = "zenctl";

/// Stable, deterministic fingerprint of the extension files bundled into this
/// binary. The extension computes the same hash at startup (background.js
/// `computeFingerprint()`) and sends it in its Hello message. The host
/// compares them; a mismatch means Zen has loaded a stale copy of the
/// extension and the user needs to re-add it in about:debugging (or run
/// `zenctl install` again to re-extract + reload).
///
/// Algorithm: for each `(rel_path, content)` in `EXTENSION_FILES`, in the
/// order they're declared, feed `rel_path || 0x00 || content || 0x00` into a
/// single SHA-256. Output is hex-lowercase.
pub fn extension_fingerprint() -> &'static str {
    static FP: OnceLock<String> = OnceLock::new();
    FP.get_or_init(|| {
        let mut h = Sha256::new();
        for (rel, content) in EXTENSION_FILES {
            h.update(rel.as_bytes());
            h.update([0u8]);
            h.update(content.as_bytes());
            h.update([0u8]);
        }
        hex::encode(h.finalize())
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum ManifestVariant {
    Basic,
    Privileged,
}

impl ManifestVariant {
    pub fn filename(self) -> &'static str {
        match self {
            Self::Basic => "manifest-basic.json",
            Self::Privileged => "manifest-privileged.json",
        }
    }
}

pub struct InstallOpts {
    pub variant: ManifestVariant,
    pub ext_dir: Option<PathBuf>,
    pub link: bool,
    pub check: bool,
    pub no_kill: bool,
}

pub async fn run(opts: InstallOpts) -> Result<()> {
    if opts.check {
        return report_state();
    }
    let binary = std::env::current_exe().context("could not resolve own binary path")?;
    let manifest = manifest_path()?;

    // 1. Native-messaging manifest — idempotent.
    if let Some(dir) = manifest.parent() {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("create directory {}", dir.display()))?;
    }
    let manifest_body = serde_json::json!({
        "name":                HOST_NAME,
        "description":         "zenctl native messaging host for Zen Browser",
        "path":                binary.to_str().context("binary path is not valid UTF-8")?,
        "type":                "stdio",
        "allowed_extensions":  [EXTENSION_ID],
    });
    std::fs::write(&manifest, serde_json::to_string_pretty(&manifest_body)?)
        .with_context(|| format!("write {}", manifest.display()))?;

    // 2. Extension dir — either symlink to repo (--link) or extract.
    let ext = if opts.link {
        let xdg = opts.ext_dir.unwrap_or(xdg_ext_dir()?);
        symlink_repo_extension(&xdg)?
    } else {
        let xdg = opts.ext_dir.unwrap_or(xdg_ext_dir()?);
        extract_extension(&xdg)?;
        xdg
    };
    link_manifest(opts.variant, &ext)?;

    // 3. Kill the running host so the next connection respawns it with the
    //    freshly-extracted code. Skipped via --no-kill.
    let mut killed = false;
    if !opts.no_kill {
        killed = pkill_host();
    }

    // 4. Compact status output.
    let fingerprint = extension_fingerprint();
    println!("✓ host manifest    → {}", manifest.display());
    println!("✓ extension dir    → {}", ext.display());
    println!(
        "✓ variant          → {} (fingerprint {})",
        opts.variant.filename(),
        &fingerprint[..12]
    );
    if killed {
        println!("✓ killed previous native host");
    }
    if !opts.link {
        println!();
        println!("about:debugging — Remove the existing zenctl entry, then");
        println!(
            "Load Temporary Add-on… → {}",
            ext.join("manifest.json").display()
        );
    }

    if opts.variant == ManifestVariant::Privileged {
        println!();
        println!("(privileged variant requires `extensions.experiments.enabled = true`)");
    }
    Ok(())
}

/// `zenctl install --check` — print resolved paths and current state without
/// making changes. Useful for diagnosing "is anything actually installed?"
fn report_state() -> Result<()> {
    let manifest = manifest_path()?;
    let ext = xdg_ext_dir()?;
    let fingerprint = extension_fingerprint();
    println!("manifest path     → {}", manifest.display());
    println!(
        "  exists          → {}",
        if manifest.exists() { "yes" } else { "no" }
    );
    println!("extension dir     → {}", ext.display());
    println!(
        "  exists          → {}",
        if ext.exists() { "yes" } else { "no" }
    );
    let active = ext.join("manifest.json");
    if let Ok(target) = std::fs::read_link(&active) {
        println!("  variant symlink → {}", target.display());
    } else if active.exists() {
        println!("  variant         → manifest.json present (not a symlink)");
    }
    println!("bundled fingerprint → {}", &fingerprint[..16]);
    Ok(())
}

fn pkill_host() -> bool {
    // Best-effort. Match the same process pattern as AGENTS.md documents.
    let status = std::process::Command::new("pkill")
        .args(["-f", "zenctl.*Mozilla/NativeMessagingHosts/zenctl.json"])
        .status();
    matches!(status, Ok(s) if s.success())
}

fn symlink_repo_extension(dest: &Path) -> Result<PathBuf> {
    // Find repo extension/: walk up from CWD looking for a sibling.
    let mut cur = std::env::current_dir().context("get current dir")?;
    let src = loop {
        let candidate = cur.join("extension");
        if candidate.join("manifest-basic.json").exists() {
            break candidate;
        }
        if !cur.pop() {
            anyhow::bail!("--link could not find a repo extension/ directory");
        }
    };
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if dest.exists() || dest.symlink_metadata().is_ok() {
        if dest.is_dir() && std::fs::read_link(dest).is_err() {
            std::fs::remove_dir_all(dest).ok();
        } else {
            std::fs::remove_file(dest).ok();
        }
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&src, dest)
        .with_context(|| format!("symlink {} -> {}", dest.display(), src.display()))?;
    #[cfg(not(unix))]
    anyhow::bail!("--link is only supported on Unix");
    Ok(dest.to_path_buf())
}

pub fn link_manifest(variant: ManifestVariant, ext_dir: &Path) -> Result<()> {
    let src_name = variant.filename();
    let src = ext_dir.join(src_name);
    if !src.exists() {
        anyhow::bail!("missing {}", src.display());
    }
    let dest = ext_dir.join("manifest.json");
    if dest.exists() || dest.symlink_metadata().is_ok() {
        std::fs::remove_file(&dest).with_context(|| format!("remove {}", dest.display()))?;
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(src_name, &dest)
        .with_context(|| format!("symlink {} -> {}", dest.display(), src_name))?;
    #[cfg(not(unix))]
    std::fs::copy(&src, &dest)
        .with_context(|| format!("copy {} -> {}", src.display(), dest.display()))?;
    Ok(())
}

const EXTENSION_FILES: &[(&str, &str)] = &[
    (
        "manifest-basic.json",
        include_str!("../../../extension/manifest-basic.json"),
    ),
    (
        "manifest-privileged.json",
        include_str!("../../../extension/manifest-privileged.json"),
    ),
    (
        "api/zenChrome.js",
        include_str!("../../../extension/api/zenChrome.js"),
    ),
    (
        "api/zenChrome.json",
        include_str!("../../../extension/api/zenChrome.json"),
    ),
    (
        "api/zenPrefs.js",
        include_str!("../../../extension/api/zenPrefs.js"),
    ),
    (
        "api/zenPrefs.json",
        include_str!("../../../extension/api/zenPrefs.json"),
    ),
    (
        "src/background.js",
        include_str!("../../../extension/src/background.js"),
    ),
    (
        "src/options.html",
        include_str!("../../../extension/src/options.html"),
    ),
    (
        "src/options.js",
        include_str!("../../../extension/src/options.js"),
    ),
];

fn xdg_ext_dir() -> Result<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default();
            home.join(".local/share")
        });
    Ok(base.join("zenctl").join("extension"))
}

pub fn extract_extension(dest: &Path) -> Result<()> {
    for (rel, content) in EXTENSION_FILES {
        let file = dest.join(rel);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        std::fs::write(&file, content).with_context(|| format!("write {}", file.display()))?;
    }
    Ok(())
}

pub fn find_extension_dir() -> Result<PathBuf> {
    // Prefer XDG install dir.
    if let Ok(xdg) = xdg_ext_dir() {
        if xdg.join("manifest-basic.json").exists() || xdg.join("manifest-privileged.json").exists()
        {
            return Ok(xdg);
        }
    }
    // Fall back to CWD walk (dev use).
    let mut cur = std::env::current_dir().context("get current directory")?;
    loop {
        let candidate = cur.join("extension");
        if candidate.join("manifest-basic.json").exists()
            || candidate.join("manifest-privileged.json").exists()
        {
            return Ok(candidate);
        }
        if !cur.pop() {
            anyhow::bail!(
                "could not locate extension/ directory. Pass --ext-dir or run `zenctl install` first."
            );
        }
    }
}

fn manifest_path() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .context("$HOME not set")?;
        Ok(home
            .join("Library/Application Support/Mozilla/NativeMessagingHosts")
            .join(format!("{HOST_NAME}.json")))
    }

    #[cfg(target_os = "linux")]
    {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .context("$HOME not set")?;
        Ok(home
            .join(".mozilla/native-messaging-hosts")
            .join(format!("{HOST_NAME}.json")))
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        anyhow::bail!("zenctl install is not yet supported on this platform");
    }
}

pub fn capabilities() -> Vec<Capability> {
    use Method::*;
    let entries: &[(Method, Tier, bool, Option<&str>)] = &[
        (Status, Tier::StableApi, true, None),
        (Capabilities, Tier::StableApi, true, None),
        (
            ExtReload,
            Tier::StableApi,
            true,
            Some("extension maintenance"),
        ),
        (
            ExtDebug,
            Tier::StableApi,
            true,
            Some("toggle extension verbose logging"),
        ),
        (
            Watch,
            Tier::StableApi,
            true,
            Some("stream live tab/window events"),
        ),
        (BookmarksList, Tier::StableApi, true, None),
        (BookmarksCreate, Tier::StableApi, true, None),
        (BookmarksUpdate, Tier::StableApi, true, None),
        (BookmarksRemove, Tier::StableApi, true, None),
        (BookmarksMove, Tier::StableApi, true, None),
        (BookmarksSearch, Tier::StableApi, true, None),
        (TabsList, Tier::StableApi, true, None),
        (TabsFind, Tier::StableApi, true, None),
        (TabsOpen, Tier::StableApi, true, None),
        (TabsClose, Tier::StableApi, true, None),
        (TabsMove, Tier::StableApi, true, None),
        (TabsActivate, Tier::StableApi, true, None),
        (TabsReload, Tier::StableApi, true, None),
        (TabsDuplicate, Tier::StableApi, true, None),
        (
            TabsDiscard,
            Tier::StableApi,
            true,
            Some("unload tab from memory"),
        ),
        (TabsSetMuted, Tier::StableApi, true, None),
        (TabsSetPinned, Tier::StableApi, true, None),
        (
            TabsScreenshot,
            Tier::StableApi,
            true,
            Some("viewport PNG/JPEG via captureTab; --full-page scroll-stitches tiles"),
        ),
        (TabsZoom, Tier::StableApi, true, Some("get/set tab zoom")),
        (
            TabsReader,
            Tier::StableApi,
            true,
            Some("toggle reader mode"),
        ),
        (TabsGoBack, Tier::StableApi, true, None),
        (TabsGoForward, Tier::StableApi, true, None),
        (WindowsList, Tier::StableApi, true, None),
        (WindowsFocus, Tier::StableApi, true, None),
        (WindowsClose, Tier::StableApi, true, None),
        (WindowsCreate, Tier::StableApi, true, None),
        (
            WindowsUpdate,
            Tier::StableApi,
            true,
            Some("window state/geometry"),
        ),
        (HistorySearch, Tier::StableApi, true, None),
        (HistoryDelete, Tier::StableApi, true, None),
        (HistoryAdd, Tier::StableApi, true, None),
        (HistoryGetVisits, Tier::StableApi, true, None),
        (DownloadsList, Tier::StableApi, true, None),
        (DownloadsCancel, Tier::StableApi, true, None),
        (DownloadsStart, Tier::StableApi, true, None),
        (DownloadsPause, Tier::StableApi, true, None),
        (DownloadsResume, Tier::StableApi, true, None),
        (CookiesGet, Tier::StableApi, true, None),
        (CookiesSet, Tier::StableApi, true, None),
        (CookiesRemove, Tier::StableApi, true, None),
        (
            SessionsClosed,
            Tier::StableApi,
            true,
            Some("recently closed tabs/windows"),
        ),
        (
            SessionsRestore,
            Tier::StableApi,
            true,
            Some("reopen a closed tab/window"),
        ),
        (
            DataClear,
            Tier::StableApi,
            true,
            Some("clear cache/cookies/history/etc"),
        ),
        (
            ContainersList,
            Tier::StableApi,
            true,
            Some("contextual identities"),
        ),
        (ContainersCreate, Tier::StableApi, true, None),
        (ContainersUpdate, Tier::StableApi, true, None),
        (ContainersRemove, Tier::StableApi, true, None),
        (
            FindInPage,
            Tier::StableApi,
            true,
            Some("find text in a page"),
        ),
        (
            FindClear,
            Tier::StableApi,
            true,
            Some("clear find highlights"),
        ),
        (
            SearchList,
            Tier::StableApi,
            true,
            Some("installed search engines"),
        ),
        (
            SearchQuery,
            Tier::StableApi,
            true,
            Some("run a search in a tab"),
        ),
        (
            PageInfo,
            Tier::StableApi,
            true,
            Some("via tabs.executeScript"),
        ),
        (
            PageText,
            Tier::StableApi,
            true,
            Some("via tabs.executeScript"),
        ),
        (
            PageSource,
            Tier::StableApi,
            true,
            Some("via tabs.executeScript"),
        ),
        (
            PageSnapshot,
            Tier::StableApi,
            true,
            Some("via tabs.executeScript"),
        ),
        (
            PageClick,
            Tier::StableApi,
            true,
            Some("via tabs.executeScript"),
        ),
        (
            PageType,
            Tier::StableApi,
            true,
            Some("via tabs.executeScript"),
        ),
        (
            PageKey,
            Tier::StableApi,
            true,
            Some("via tabs.executeScript"),
        ),
        (
            PageWait,
            Tier::StableApi,
            true,
            Some("via tabs.executeScript"),
        ),
        (
            PageEval,
            Tier::Experimental,
            true,
            Some("unsafe JavaScript eval"),
        ),
        (
            PageFrames,
            Tier::StableApi,
            true,
            Some("list frames via webNavigation"),
        ),
        (
            MediaStatus,
            Tier::StableApi,
            true,
            Some("page media controls"),
        ),
        (
            MediaPlay,
            Tier::StableApi,
            true,
            Some("page media controls"),
        ),
        (
            MediaPause,
            Tier::StableApi,
            true,
            Some("page media controls"),
        ),
        (
            MediaToggle,
            Tier::StableApi,
            true,
            Some("page media controls"),
        ),
        (
            MediaNext,
            Tier::StableApi,
            true,
            Some("page media controls"),
        ),
        (
            MediaPrevious,
            Tier::StableApi,
            true,
            Some("page media controls"),
        ),
        (
            CompactToggle,
            Tier::UiAutomation,
            true,
            Some("via zenChrome experiment"),
        ),
        (
            CompactSet,
            Tier::UiAutomation,
            true,
            Some("via zenChrome experiment"),
        ),
        (
            CompactHide,
            Tier::UiAutomation,
            true,
            Some("hide sidebar/toolbar/both"),
        ),
        (
            PrefsGet,
            Tier::Preference,
            true,
            Some("via zenPrefs experiment"),
        ),
        (
            PrefsSet,
            Tier::Preference,
            true,
            Some("via zenPrefs experiment"),
        ),
        (
            PrefsList,
            Tier::Preference,
            true,
            Some("via zenPrefs experiment"),
        ),
        (
            PrefsClear,
            Tier::Preference,
            true,
            Some("via zenPrefs experiment"),
        ),
        (
            WorkspaceSwitch,
            Tier::UiAutomation,
            true,
            Some("via zenChrome experiment"),
        ),
        (
            WorkspaceCreate,
            Tier::UiAutomation,
            true,
            Some("via zenChrome experiment"),
        ),
        (
            WorkspaceRemove,
            Tier::UiAutomation,
            true,
            Some("via zenChrome experiment"),
        ),
        (
            WorkspaceRename,
            Tier::UiAutomation,
            true,
            Some("via zenChrome experiment"),
        ),
        (
            WorkspaceSetIcon,
            Tier::UiAutomation,
            true,
            Some("via zenChrome experiment"),
        ),
        (
            WorkspaceSetContainer,
            Tier::UiAutomation,
            true,
            Some("via zenChrome experiment"),
        ),
        (
            WorkspaceReorder,
            Tier::UiAutomation,
            true,
            Some("via zenChrome experiment"),
        ),
        (
            WorkspaceMoveTab,
            Tier::UiAutomation,
            true,
            Some("move existing tabs (by id/url) into a workspace"),
        ),
        (
            WorkspaceList,
            Tier::UiAutomation,
            true,
            Some("via zenChrome experiment"),
        ),
        (
            WorkspaceUnload,
            Tier::UiAutomation,
            true,
            Some("unload tabs in a workspace"),
        ),
        (
            WorkspaceUnloadAll,
            Tier::UiAutomation,
            true,
            Some("unload tabs in all other workspaces"),
        ),
        (
            ShortcutsRead,
            Tier::ProfileFile,
            true,
            Some("reads zen-keyboard-shortcuts.json"),
        ),
        (
            ShortcutsWrite,
            Tier::ProfileFile,
            true,
            Some("writes zen-keyboard-shortcuts.json"),
        ),
        (
            GlanceClose,
            Tier::UiAutomation,
            true,
            Some("via zenChrome experiment"),
        ),
        (
            GlanceCloseAll,
            Tier::UiAutomation,
            true,
            Some("close all open glances"),
        ),
        (
            GlanceList,
            Tier::UiAutomation,
            true,
            Some("list open glances"),
        ),
        (
            GlanceExpand,
            Tier::UiAutomation,
            true,
            Some("via zenChrome experiment"),
        ),
        (
            GlanceOpen,
            Tier::UiAutomation,
            true,
            Some("open glance on a URL"),
        ),
        (
            UrlbarSearch,
            Tier::UiAutomation,
            true,
            Some("populate/submit the address bar via gURLBar"),
        ),
        (
            UrlbarClose,
            Tier::UiAutomation,
            true,
            Some("revert + close the address bar (Esc behavior)"),
        ),
        (
            UrlbarActionsList,
            Tier::UiAutomation,
            true,
            Some("list Zen URL-bar global actions"),
        ),
        (
            UrlbarActionsRun,
            Tier::UiAutomation,
            true,
            Some("run a Zen URL-bar global action"),
        ),
        (
            EssentialsList,
            Tier::UiAutomation,
            true,
            Some("list essential (zen-essential) tabs across windows"),
        ),
        (
            EssentialsAdd,
            Tier::UiAutomation,
            true,
            Some("add tabs (by id/url) to essentials"),
        ),
        (
            EssentialsRemove,
            Tier::UiAutomation,
            true,
            Some("remove tabs from essentials (optionally keep pinned)"),
        ),
        (
            EssentialsReset,
            Tier::UiAutomation,
            true,
            Some("reset a pinned/essential tab to its stored URL"),
        ),
        (
            EssentialsReplaceUrl,
            Tier::UiAutomation,
            true,
            Some("commit a pinned tab's current URL as its stored URL"),
        ),
        (
            Share,
            Tier::UiAutomation,
            true,
            Some("native share dialog \u{2014} Windows/macOS only"),
        ),
        (
            ShareCan,
            Tier::UiAutomation,
            true,
            Some("whether native share is supported on this platform"),
        ),
        (
            ModsList,
            Tier::UiAutomation,
            true,
            Some("installed Zen mods"),
        ),
        (
            ModsInstall,
            Tier::UiAutomation,
            true,
            Some("install a mod by id"),
        ),
        (ModsRemove, Tier::UiAutomation, true, None),
        (ModsEnable, Tier::UiAutomation, true, None),
        (ModsDisable, Tier::UiAutomation, true, None),
        (
            ModsPreferences,
            Tier::UiAutomation,
            true,
            Some("read a mod's preferences"),
        ),
        (
            ModsSetPreference,
            Tier::UiAutomation,
            true,
            Some("set a mod's preference value (UX sugar over PrefsSet)"),
        ),
        (
            FoldersList,
            Tier::UiAutomation,
            true,
            Some("list pinned-tab folders (zen-folder elements)"),
        ),
        (
            FoldersCreate,
            Tier::UiAutomation,
            true,
            Some("create an empty folder via gZenFolders.createFolder"),
        ),
        (
            FoldersDelete,
            Tier::UiAutomation,
            true,
            Some("delete a folder and its tabs"),
        ),
        (
            FoldersRename,
            Tier::UiAutomation,
            true,
            Some("rename a folder by id"),
        ),
        (
            FoldersCollapse,
            Tier::UiAutomation,
            true,
            Some("expand or collapse a folder"),
        ),
        (
            FoldersAddTab,
            Tier::UiAutomation,
            true,
            Some("add existing tabs (by url) into a folder"),
        ),
        (
            FoldersSetIcon,
            Tier::UiAutomation,
            true,
            Some("set a folder's emoji/svg icon"),
        ),
        (
            FoldersCreateSubfolder,
            Tier::UiAutomation,
            true,
            Some("create a nested subfolder"),
        ),
        (
            FoldersUnpack,
            Tier::UiAutomation,
            true,
            Some("ungroup all tabs out of a folder"),
        ),
        (
            FoldersUnload,
            Tier::UiAutomation,
            true,
            Some("unload (discard) every tab in a folder"),
        ),
        (
            FoldersMoveToWorkspace,
            Tier::UiAutomation,
            true,
            Some("move a folder into another workspace"),
        ),
        (
            FoldersConvertToWorkspace,
            Tier::UiAutomation,
            true,
            Some("convert a folder into a new workspace"),
        ),
        (
            LiveFoldersList,
            Tier::UiAutomation,
            true,
            Some("list Zen live folders"),
        ),
        (
            LiveFoldersCreate,
            Tier::UiAutomation,
            true,
            Some("create RSS/GitHub live folders"),
        ),
        (
            LiveFoldersDelete,
            Tier::UiAutomation,
            true,
            Some("delete a live folder"),
        ),
        (
            LiveFoldersRefresh,
            Tier::UiAutomation,
            true,
            Some("refresh a live folder"),
        ),
        (
            LiveFoldersPause,
            Tier::UiAutomation,
            true,
            Some("pause a live folder auto-fetch timer"),
        ),
        (
            LiveFoldersResume,
            Tier::UiAutomation,
            true,
            Some("resume a live folder auto-fetch timer"),
        ),
        (
            BoostsList,
            Tier::UiAutomation,
            true,
            Some("list Zen Boosts (runtime-gated by Zen version)"),
        ),
        (
            BoostsCreate,
            Tier::UiAutomation,
            true,
            Some("create a Zen Boost for a domain"),
        ),
        (
            BoostsDelete,
            Tier::UiAutomation,
            true,
            Some("delete a Zen Boost"),
        ),
        (
            BoostsActivate,
            Tier::UiAutomation,
            true,
            Some("make a Boost active for its domain"),
        ),
        (
            BoostsToggle,
            Tier::UiAutomation,
            true,
            Some("toggle active Boost for its domain"),
        ),
        (
            BoostsUpdate,
            Tier::UiAutomation,
            true,
            Some("update a Boost's CSS and visual data"),
        ),
        (
            SplitViewCreate,
            Tier::UiAutomation,
            true,
            Some("via zenChrome experiment"),
        ),
        (
            SplitUnsplit,
            Tier::UiAutomation,
            true,
            Some("unsplit current view"),
        ),
        (
            SplitViewList,
            Tier::UiAutomation,
            true,
            Some("list active split groups + layout tree"),
        ),
        (
            SplitViewAddTab,
            Tier::UiAutomation,
            true,
            Some("add tabs to the active/existing split group"),
        ),
        (
            SplitViewSetLayout,
            Tier::UiAutomation,
            true,
            Some("change split layout/grid type"),
        ),
        (
            SplitViewResize,
            Tier::UiAutomation,
            true,
            Some("resize children of the active split layout tree"),
        ),
        (
            SplitViewRearrange,
            Tier::UiAutomation,
            true,
            Some("toggle split pane drag-reorder mode"),
        ),
        (
            WindowSyncForce,
            Tier::UiAutomation,
            true,
            Some("force cross-window workspace sync via gZenWorkspaces"),
        ),
        (
            ShortcutsReset,
            Tier::UiAutomation,
            true,
            Some("reset keyboard shortcuts to Zen defaults"),
        ),
        (
            SessionList,
            Tier::ProfileFile,
            true,
            Some("reads sessionstore.jsonlz4"),
        ),
        (
            SessionBackup,
            Tier::ProfileFile,
            true,
            Some("copies sessionstore to backup"),
        ),
        (
            TabDetach,
            Tier::StableApi,
            true,
            Some("detach a tab into its own window"),
        ),
        (
            TabGroup,
            Tier::StableApi,
            true,
            Some("uses browser.tabs.group when available"),
        ),
        (
            TabUngroup,
            Tier::StableApi,
            true,
            Some("uses browser.tabs.ungroup when available"),
        ),
        (
            SessionRestoreWindow,
            Tier::StableApi,
            true,
            Some("restore a recently closed window by session id or latest window"),
        ),
        (
            SessionRestoreTab,
            Tier::StableApi,
            true,
            Some("restore a recently closed tab by session id or latest tab"),
        ),
    ];
    entries
        .iter()
        .map(|(m, t, available, reason)| Capability {
            method: *m,
            tier: *t,
            available: *available,
            reason: reason.map(|s| s.to_string()),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_path_linux() {
        if !cfg!(target_os = "linux") {
            return;
        }
        let path = manifest_path().expect("manifest_path");
        let s = path.to_string_lossy();
        assert!(
            s.contains("native-messaging-hosts") && s.ends_with("zenctl.json"),
            "unexpected manifest path: {s}"
        );
    }

    #[test]
    fn test_manifest_path_macos() {
        if !cfg!(target_os = "macos") {
            return;
        }
        let path = manifest_path().expect("manifest_path");
        let s = path.to_string_lossy();
        assert!(
            s.contains("NativeMessagingHosts") && s.ends_with("zenctl.json"),
            "unexpected manifest path: {s}"
        );
    }

    #[test]
    fn test_capabilities_non_empty() {
        let caps = capabilities();
        assert!(!caps.is_empty(), "capabilities should not be empty");
        // Status and Capabilities should always be available.
        let status = caps
            .iter()
            .find(|c| c.method == Method::Status)
            .expect("Status cap");
        assert!(status.available, "Status should be available");
        let cap_list = caps
            .iter()
            .find(|c| c.method == Method::Capabilities)
            .expect("Capabilities cap");
        assert!(cap_list.available, "Capabilities should be available");
    }

    #[test]
    fn test_every_capability_is_routed() {
        // Drift guard: every method advertised by capabilities() must be
        // routed by host::dispatch — either forwarded to the extension
        // (is_ext_method) or handled directly by the host.
        let host_handled = [
            Method::Status,
            Method::Capabilities,
            Method::Watch,
            Method::SessionList,
            Method::SessionBackup,
            Method::ShortcutsRead,
            Method::ShortcutsWrite,
        ];
        for cap in capabilities() {
            let routed =
                crate::host::is_ext_method(cap.method) || host_handled.contains(&cap.method);
            assert!(
                routed,
                "capability {:?} has no route in host::dispatch",
                cap.method
            );
        }
    }

    #[test]
    fn test_capabilities_unique_methods() {
        let caps = capabilities();
        let names: Vec<String> = caps.iter().map(|c| format!("{:?}", c.method)).collect();
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            names.len(),
            sorted.len(),
            "capabilities must have unique methods"
        );
    }

    #[test]
    fn test_ext_capabilities_have_background_handlers() {
        let background = EXTENSION_FILES
            .iter()
            .find(|(rel, _)| *rel == "src/background.js")
            .map(|(_, content)| *content)
            .expect("background.js bundled");

        for cap in capabilities() {
            if !crate::host::is_ext_method(cap.method) {
                continue;
            }
            let method = serde_json::to_value(cap.method)
                .expect("serialize method")
                .as_str()
                .expect("method serializes as string")
                .to_string();
            let needle = format!("async {method}");
            assert!(
                background.contains(&needle),
                "extension-routed capability {:?} missing background handler `{}`",
                cap.method,
                method
            );
        }
    }
}
