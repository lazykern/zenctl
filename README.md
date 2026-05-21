# zenctl

zenctl is a command-line control plane for Zen Browser. It uses a native host and WebExtension to let shell scripts inspect and modify browser state.

It supports standard WebExtension features, Zen-specific UI controls exposed through a privileged experiment API, and selected profile-file operations.

This project is unstable and still in development.

## Build

```sh
cargo build
```

During development, use the local binary explicitly:

```sh
./target/debug/zenctl --help
```

## Install / update

Register the native messaging host and install the extension:

```sh
./target/debug/zenctl install
```

The installer is idempotent. Run it again after changing extension files or rebuilding the host.

For extension development, link the browser extension directly to this checkout:

```sh
./target/debug/zenctl install --link
```

Check what is installed without changing anything:

```sh
./target/debug/zenctl install --check
```

## Basic use

```sh
zenctl status
zenctl ext status
zenctl capabilities
```

State snapshot for automation:

```sh
zenctl snapshot --json
```

Tabs and windows:

```sh
zenctl tabs list
zenctl tabs find --title-contains GitHub
zenctl tabs open https://example.com
zenctl tabs detach 123
zenctl tabs group 123 456
zenctl tabs ungroup 123 456
zenctl windows list
```

Zen features:

```sh
zenctl workspace list
zenctl workspace switch <uuid>
zenctl compact toggle
zenctl split list
zenctl glance list
zenctl boosts list
zenctl mods list
zenctl live-folders list
```

Recently closed sessions:

```sh
zenctl sessions closed
zenctl sessions restore
zenctl sessions restore-window
zenctl sessions restore-tab <session-id>
```

Wait for conditions (no shell sleeps):

```sh
zenctl wait tab-loaded --active
zenctl wait url-contains github --active
zenctl wait title-contains "Dashboard" --active
zenctl wait text loaded --active
```

Page automation:

```sh
zenctl page info --active
zenctl page text --active
zenctl page click --ref f0:e2
zenctl page type-ref f0:e4 "hello"
zenctl find "needle"
zenctl media status
```

Checkpoints and profile-backed data:

```sh
zenctl checkpoint create
zenctl checkpoint list
zenctl prefs get zen.view.compact
zenctl prefs set zen.view.compact.hide-toolbar true
zenctl session list
zenctl shortcuts read
```

Most commands support `--json` for scripting.

## Runtime notes

The privileged extension variant is required for Zen-specific commands such as workspaces, split view, boosts, mods, glance, folders, and live folders.

Some commands depend on APIs exposed by the installed Zen/Firefox build. For example, `tabs group` and `tabs ungroup` call Firefox tab-group WebExtension APIs when they are available; otherwise zenctl returns a clear “unavailable” error.

If commands stop reaching the browser, start with:

```sh
zenctl status
zenctl ext status
zenctl ext reload
```

## Development checks

```sh
node --check extension/src/background.js
cargo test
```
