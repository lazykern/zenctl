#!/usr/bin/env bash
# smoke-test.sh — end-to-end smoke test of zenctl with a real Zen Browser
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ZENCTL="${SCRIPT_DIR}/target/release/zenctl"

GREEN='\033[0;32m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'
PASS=0
FAIL=0
TOTAL=0

pass() { PASS=$((PASS+1)); echo -e "  ${GREEN}✓ PASS${NC} $1"; }
fail() { FAIL=$((FAIL+1)); echo -e "  ${RED}✗ FAIL${NC} $1"; }
check() {
    TOTAL=$((TOTAL+1))
    local name="$1" expected="$2" actual="$3"
    if echo "$actual" | grep -q "$expected"; then
        pass "$name"
    else
        fail "$name (expected: $expected, got: ${actual:0:120})"
    fi
}

echo "=========================================="
echo "  zenctl Smoke Test (real Zen Browser)"
echo "=========================================="
echo ""

# ──────────────────────────────────────
# Cleanup handler
# ──────────────────────────────────────
cleanup() {
    echo ""
    echo "--- Cleanup ---"
    # Kill Zen + web-ext
    pkill -f "zen-browser-bin" 2>/dev/null || true
    pkill -f "web-ext" 2>/dev/null || true
    pkill -f "playwright-cli" 2>/dev/null || true
    sleep 1
    rm -f /tmp/zenctl.sock

    # Kill Firefox helper if web-ext spawned one
    pkill -f "firefox" 2>/dev/null || true
}
trap cleanup EXIT

# Ensure we start clean
cleanup

# ──────────────────────────────────────
# Step 1: Build
# ──────────────────────────────────────
echo "--- Step 1: Build release ---"
(cd "${SCRIPT_DIR}" && cargo build --release 2>&1 | tail -3)
echo ""

# ──────────────────────────────────────
# Step 2: Run unit tests
# ──────────────────────────────────────
echo "--- Step 2: Unit tests ---"
(cd "${SCRIPT_DIR}" && cargo test 2>&1) | tail -5
echo ""

# ──────────────────────────────────────
# Step 3: Install native messaging host
# ──────────────────────────────────────
echo "--- Step 3: zenctl install ---"
"${ZENCTL}" install 2>&1
echo ""

# ──────────────────────────────────────
# Step 4: Launch Zen Browser via web-ext
# ──────────────────────────────────────
echo "--- Step 4: Launch Zen with extension via web-ext ---"
ZEN_BIN="/opt/zen-browser-bin/zen-bin"
if [ ! -x "$ZEN_BIN" ]; then
    ZEN_BIN="$(which zen-browser 2>/dev/null || echo '')"
    if [ -z "$ZEN_BIN" ]; then
        fail "Zen Browser binary not found"
        exit 1
    fi
fi
echo "• Zen binary: ${ZEN_BIN}"

# Launch Zen with the extension via web-ext, output to file so we can monitor
# Use --no-reload to prevent file-watching issues
npx web-ext run --source-dir "${SCRIPT_DIR}/extension" \
    --firefox "${ZEN_BIN}" \
    --firefox-profile "$(mktemp -d /tmp/zenctl-web-ext-XXXXXX)" \
    --no-reload \
    --verbose 2>&1 &
WEB_EXT_PID=$!
echo "• web-ext PID: ${WEB_EXT_PID}"

# Wait for the Unix socket to appear (zenctl host creates this)
echo -n "• Waiting for zenctl socket"
SOCKET_WAIT=30
for i in $(seq 1 ${SOCKET_WAIT}); do
    if [ -S /tmp/zenctl.sock ]; then
        echo " ready (${i}s)"
        break
    fi
    if ! kill -0 ${WEB_EXT_PID} 2>/dev/null; then
        echo " FAILED (web-ext died after ${i}s)"
        fail "web-ext process died before socket appeared"
        exit 1
    fi
    echo -n "."
    sleep 1
done

if [ ! -S /tmp/zenctl.sock ]; then
    echo " TIMEOUT after ${SOCKET_WAIT}s"
    fail "zenctl socket never appeared"
    exit 1
fi

# Give the extension a moment to handshake with the host
sleep 2
echo ""

# ──────────────────────────────────────
# Step 5: Smoke test zenctl commands
# ──────────────────────────────────────
echo "--- Step 5: Smoke tests ---"
echo ""

echo "▶ Test: zenctl status"
STATUS=$("${ZENCTL}" status 2>&1 || true)
echo "${STATUS}"
check "status shows host version"  "protocol v"  "${STATUS}"
check "status shows extension"    "extension"    "${STATUS}"
check "status shows zen"          "zen:"         "${STATUS}"
echo ""

echo "▶ Test: zenctl status --json"
JSON_STATUS=$("${ZENCTL}" --json status 2>&1 || true)
check "JSON has daemon_version"   "daemon_version"  "${JSON_STATUS}"
check "JSON has protocol_version" "protocol_version" "${JSON_STATUS}"
check "JSON has extension_connected" "extension_connected" "${JSON_STATUS}"
check "JSON has zen_running"      "zen_running"     "${JSON_STATUS}"
echo ""

echo "▶ Test: zenctl capabilities"
CAPS=$("${ZENCTL}" --json capabilities 2>&1 || true)
check "capabilities is JSON array" "^\["   "${CAPS}"
check "has Status capability"     "Status"  "${CAPS}"
check "has TabsList capability"   "TabsList" "${CAPS}"
echo ""

echo "▶ Test: zenctl tabs list"
TABS=$("${ZENCTL}" --json tabs list 2>&1 || true)
check "tabs list returns array"  "^\["  "${TABS}"
echo ""

echo "▶ Test: zenctl windows list"
WINS=$("${ZENCTL}" --json windows list 2>&1 || true)
check "windows list returns array" "^\[" "${WINS}"
echo ""

echo "▶ Test: zenctl bookmarks list"
BMS=$("${ZENCTL}" --json bookmarks list 2>&1 || true)
check "bookmarks list returns array" "^\[" "${BMS}"
echo ""

echo "▶ Test: zenctl tabs open (about:blank)"
OPEN=$("${ZENCTL}" tabs open "about:blank" 2>&1 || true)
check "tabs open succeeds" "id=" "${OPEN}"
echo ""

echo "▶ Test: zenctl prefs list"
PREFS=$("${ZENCTL}" --json prefs list --prefix "extensions.experiments" 2>&1 || true)
# This might fail if the pref API isn't available (needs privileged extension)
# That's OK - it's a soft test
echo "  (prefs: ${PREFS:0:120})"
echo ""

echo "▶ Test: zenctl version (direct invoke)"
VERSION=$("${ZENCTL}" --version 2>&1 || true)
check "version is 0.1.0" "0.1.0" "${VERSION}"
echo ""

echo "▶ Check: binary is statically linked (no missing deps)"
if ldd "${ZENCTL}" 2>&1 | grep -iq "not found"; then
    fail "zenctl binary has missing shared library dependencies"
else
    pass "zenctl binary dependencies OK"
fi
echo ""

# ──────────────────────────────────────
# Step 6: Drift guard — every ext capability must have a handler
# ──────────────────────────────────────
echo "--- Step 6: Extension handler drift guard ---"
if (cd "${SCRIPT_DIR}" && cargo test -p zenctl-cli install::tests::test_ext_capabilities_have_background_handlers --quiet >/tmp/zenctl-drift-test.log 2>&1); then
    pass "All extension-routed capabilities have background.js handlers"
else
    cat /tmp/zenctl-drift-test.log
    fail "Extension handler drift guard failed"
fi
echo ""

# ──────────────────────────────────────
# Step 7: Additional smoke tests
# ──────────────────────────────────────
echo "--- Step 7: Additional command tests ---"
echo ""

echo "▶ Test: zenctl containers list"
CTRS=$("${ZENCTL}" --json containers list 2>&1 || true)
check "containers list returns array" "^\[" "${CTRS}"
echo ""

echo "▶ Test: zenctl workspace list"
WSPC=$("${ZENCTL}" --json workspace list 2>&1 || true)
check "workspace list has workspaces" "workspaces" "${WSPC}"
echo ""

echo "▶ Test: zenctl split list"
SPLIT_LIST=$("${ZENCTL}" --json split list 2>&1 || true)
check "split list returns groups" "groups" "${SPLIT_LIST}"
echo ""

echo "▶ Test: zenctl folders list"
FOLDERS_LIST=$("${ZENCTL}" --json folders list 2>&1 || true)
check "folders list returns folders" "folders" "${FOLDERS_LIST}"
echo ""

echo "▶ Test: zenctl live-folders list"
LIVE_FOLDERS=$("${ZENCTL}" --json live-folders list 2>&1 || true)
check "live-folders list returns folders" "folders" "${LIVE_FOLDERS}"
echo ""

echo "▶ Test: zenctl urlbar actions"
URLBAR_ACTIONS=$("${ZENCTL}" --json urlbar actions 2>&1 || true)
check "urlbar actions returns actions" "actions" "${URLBAR_ACTIONS}"
echo ""

echo "▶ Test: zenctl boosts list (may be version-gated)"
BOOSTS_LIST=$("${ZENCTL}" --json boosts list 2>&1 || true)
if echo "${BOOSTS_LIST}" | grep -q "domains"; then
    pass "boosts list returns domains"
elif echo "${BOOSTS_LIST}" | grep -qi "unavailable\|Failed to load\|gZenBoostsManager"; then
    pass "boosts list is clearly version-gated"
else
    fail "boosts list unexpected output (${BOOSTS_LIST:0:120})"
fi
echo ""

echo "▶ Test: zenctl history search (example)"
HIST=$("${ZENCTL}" --json history search "example" 2>&1 || true)
check "history search returns array" "^\[" "${HIST}"
echo ""

echo "▶ Test: zenctl search engines"
ENGS=$("${ZENCTL}" --json search engines 2>&1 || true)
check "search engines returns array" "^\[" "${ENGS}"
echo ""

echo "▶ Test: zenctl tabs screenshot (active)"
"${ZENCTL}" tabs screenshot --active -o /tmp/zenctl-smoke-test.png 2>&1 || true
if [ -s /tmp/zenctl-smoke-test.png ]; then
    pass "tabs screenshot produces output"
else
    fail "tabs screenshot produced empty file"
fi
rm -f /tmp/zenctl-smoke-test.png
echo ""

echo "▶ Test: zenctl bookmarks create (without --force)"
BM_CREATE=$("${ZENCTL}" bookmarks create "zenctl-smoke-test" --url about:blank 2>&1 || true)
# Extract the ID for cleanup
BM_ID=$(echo "${BM_CREATE}" | grep -oP 'id="\K[^"]+' || true)
if [ -n "${BM_ID}" ]; then
    pass "bookmarks create returns id"
    "${ZENCTL}" bookmarks remove "${BM_ID}" --force 2>/dev/null || true
else
    fail "bookmarks create failed (${BM_CREATE:0:80})"
fi
echo ""

echo "▶ Test: zenctl page info (active)"
PAGEINFO=$("${ZENCTL}" --json page info --active 2>&1 || true)
check "page info has url" "url" "${PAGEINFO}"
echo ""

echo "▶ Test: zenctl session backup"
BACKUP=$("${ZENCTL}" session backup 2>&1 || true)
check "session backup writes file" "wrote\|ok" "${BACKUP}"
echo ""

echo "▶ Test: zenctl shortcuts read"
SHCUT=$("${ZENCTL}" --json shortcuts read 2>&1 || true)
# Might succeed or fail depending on shortcuts file — just check it's valid output
if echo "${SHCUT}" | python3 -m json.tool >/dev/null 2>&1; then
    pass "shortcuts read returns valid JSON"
else
    echo "  (shortcuts: may be empty or not configured)"
fi
echo ""

# ──────────────────────────────────────
# Summary
# ──────────────────────────────────────
echo "=========================================="
echo "  Smoke Test Results"
echo "=========================================="
echo "  Total:  ${TOTAL}"
echo -e "  ${GREEN}Passed: ${PASS}${NC}"
echo -e "  ${RED}Failed: ${FAIL}${NC}"
echo ""

if [ "${FAIL}" -eq 0 ]; then
    echo -e "${GREEN}★ ALL SMOKE TESTS PASSED ★${NC}"
else
    echo -e "${RED}★ ${FAIL} SMOKE TEST(S) FAILED ★${NC}"
fi
echo ""

[ "${FAIL}" -eq 0 ]
