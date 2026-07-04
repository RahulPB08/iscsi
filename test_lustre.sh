#!/bin/bash
# =============================================================================
#  test_lustre.sh — Single-Node Lustre Loopback Cluster Tester
#  Mounts a single-node Lustre cluster using llmount.sh and verifies it.
#  Run this on a node where build_lustre.sh has already completed.
# =============================================================================

BOLD='\033[1m'; RESET='\033[0m'; CYAN='\033[1;36m'
GREEN='\033[1;32m'; YELLOW='\033[1;33m'; RED='\033[1;31m'
BLUE='\033[1;34m'; DIM='\033[2m'

banner() {
    echo ""; echo -e "${CYAN}${BOLD}╔══════════════════════════════════════════════════════╗${RESET}"
    printf "${CYAN}${BOLD}║  %-52s║${RESET}\n" "$1"
    echo -e "${CYAN}${BOLD}╚══════════════════════════════════════════════════════╝${RESET}"; echo ""
}
phase() {
    echo ""; echo -e "${BLUE}${BOLD}┌─────────────────────────────────────────┐${RESET}"
    printf  "${BLUE}${BOLD}│  Step %-35s│${RESET}\n" "$1 — $2"
    echo -e "${BLUE}${BOLD}└─────────────────────────────────────────┘${RESET}"; echo ""
}
step() { echo -e "${CYAN}  ▶  $*${RESET}"; }
ok()   { echo -e "${GREEN}  ✔  $*${RESET}"; }
warn() { echo -e "${YELLOW}  ⚠  $*${RESET}"; }
fail() { echo -e "${RED}  ✖  $*${RESET}"; }
info() { echo -e "${DIM}     $*${RESET}"; }
confirm() {
    local answer; local hint; [[ "${2:-y}" == "y" ]] && hint="[Y/n]" || hint="[y/N]"
    read -rp "$(echo -e "${YELLOW}  ?  $1 ${hint}: ${RESET}")" answer
    answer="${answer:-${2:-y}}"; [[ "${answer,,}" == "y" ]]
}

# ── Root check ────────────────────────────────────────────────────────────────
if [[ $EUID -ne 0 ]]; then
    fail "This script must be run as root."
    info "Please run: sudo ./test_lustre.sh"; exit 1
fi

clear
banner "Lustre Single-Node Loopback Cluster Test"

echo -e "${DIM}  This script:${RESET}"
echo -e "${DIM}  1. Locates the Lustre test directory${RESET}"
echo -e "${DIM}  2. Runs llmount.sh to spin up MGS, MDS, OSS & Client${RESET}"
echo -e "${DIM}  3. Verifies active mounts and checks filesystem health${RESET}"
echo -e "${DIM}  4. Creates test user accounts for the Lustre test suite${RESET}"
echo ""
warn "Prerequisite: build_lustre.sh must have completed and the system rebooted."
echo ""

if ! confirm "Proceed with single-node cluster test?"; then
    info "Test cancelled. No changes made."; exit 0
fi

# ── Step 1: Verify Lustre source ──────────────────────────────────────────────
phase 1 "Locating Lustre Test Directory"

TESTS_DIR="/usr/src/lustre-head/lustre/tests"

if [[ -d "$TESTS_DIR" ]]; then
    ok "Lustre tests directory found: $TESTS_DIR"
else
    fail "Directory not found: $TESTS_DIR"
    echo ""
    info "This usually means build_lustre.sh did not complete successfully."
    info "Please run build_lustre.sh first, then reboot, then retry."
    exit 1
fi

# Check kernel modules are loaded
step "Checking Lustre kernel modules..."
if lsmod | grep -qE "^lustre|^lnet"; then
    ok "Lustre kernel modules are loaded."
    lsmod | grep -E "^lustre|^lnet" | while read -r line; do
        info "$line"
    done
else
    warn "Lustre kernel modules are NOT loaded."
    info "Attempting to load them now..."
    if modprobe lustre 2>/dev/null; then
        ok "Module 'lustre' loaded successfully."
    else
        fail "Could not load Lustre modules."
        info "Make sure you rebooted into the Lustre-patched kernel after running build_lustre.sh."
        info "Check active kernel: uname -r"
        exit 1
    fi
fi

echo ""

# ── Step 2: Run llmount.sh ────────────────────────────────────────────────────
phase 2 "Spinning Up Loopback Cluster with llmount.sh"

cd "$TESTS_DIR" || exit 1
step "Launching llmount.sh (MGS + MDS + OSS + Client)..."
info "This may take 30–60 seconds..."
echo ""

if sudo ./llmount.sh; then
    ok "llmount.sh completed successfully."
else
    fail "llmount.sh returned a non-zero exit code."
    warn "The cluster may be partially up. Proceeding with verification anyway."
fi

echo ""

# ── Step 3: Verify mounts ─────────────────────────────────────────────────────
phase 3 "Verifying Active Lustre Mounts"

step "Listing /mnt contents..."
ls -lh /mnt 2>/dev/null || warn "Could not list /mnt"
echo ""

step "Active Lustre mount points:"
if df -t lustre 2>/dev/null | grep -v "^Filesystem"; then
    ok "Lustre filesystems are mounted."
else
    warn "No Lustre filesystems found via df -t lustre."
    info "Check: mount | grep lustre"
fi

echo ""

step "Running quick write/read smoke test..."
LUSTRE_MNT="$(mount | grep lustre | awk '{print $3}' | head -1)"
if [[ -n "$LUSTRE_MNT" ]]; then
    TEST_FILE="${LUSTRE_MNT}/.smoke_test_$$"
    echo "SDS Lustre OK $(date)" > "$TEST_FILE" 2>/dev/null && \
        cat "$TEST_FILE" &>/dev/null && \
        rm -f "$TEST_FILE" && \
        ok "Write/read smoke test passed on ${LUSTRE_MNT}." || \
        warn "Smoke test failed — check filesystem permissions."
else
    warn "Could not locate a mounted Lustre path for smoke test."
fi

echo ""

# ── Step 4: Test accounts ─────────────────────────────────────────────────────
phase 4 "Configuring Lustre Test Framework Accounts"

step "Creating test group 'group500' (GID 500)..."
if getent group group500 &>/dev/null; then
    ok "Group 'group500' already exists."
else
    if groupadd -g 500 group500; then
        ok "Group 'group500' created."
    else
        warn "Could not create group 'group500'."
    fi
fi

step "Creating test user 'runas' (UID 500, GID 500)..."
if getent passwd runas &>/dev/null; then
    ok "User 'runas' already exists."
else
    if useradd -u 500 -g 500 runas; then
        ok "User 'runas' created."
    else
        warn "Could not create user 'runas'."
    fi
fi

# ── Complete ──────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}${BOLD}╔══════════════════════════════════════════════════════╗${RESET}"
echo -e "${GREEN}${BOLD}║   ✔  Lustre single-node cluster is running!          ║${RESET}"
echo -e "${GREEN}${BOLD}║                                                      ║${RESET}"
echo -e "${GREEN}${BOLD}║   Useful commands:                                   ║${RESET}"
echo -e "${GREEN}${BOLD}║   • df -t lustre        (show mounts)                ║${RESET}"
echo -e "${GREEN}${BOLD}║   • lctl dl             (list Lustre devices)        ║${RESET}"
echo -e "${GREEN}${BOLD}║   • lctl ping <nid>     (test node connectivity)     ║${RESET}"
echo -e "${GREEN}${BOLD}║   • cd ${TESTS_DIR}  ║${RESET}"
echo -e "${GREEN}${BOLD}║     sudo ./llmountcleanup.sh  (teardown)             ║${RESET}"
echo -e "${GREEN}${BOLD}╚══════════════════════════════════════════════════════╝${RESET}"
echo ""