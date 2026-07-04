#!/bin/bash
# =============================================================================
#  iscsi_install.sh — iSCSI Target Engine Installer
#  Installs targetcli-fb and activates the iSCSI target service.
#  Designed for Ubuntu / Debian-based systems.
#  Run this on the iSCSI Target (VM 1) before running the Rust orchestrator.
# =============================================================================

# ── Colour palette ────────────────────────────────────────────────────────────
BOLD='\033[1m'
RESET='\033[0m'
CYAN='\033[1;36m'
GREEN='\033[1;32m'
YELLOW='\033[1;33m'
RED='\033[1;31m'
BLUE='\033[1;34m'
DIM='\033[2m'

# ── Helpers ───────────────────────────────────────────────────────────────────
banner() {
    echo ""
    echo -e "${CYAN}${BOLD}╔══════════════════════════════════════════════════════════╗${RESET}"
    printf "${CYAN}${BOLD}║  %-56s║${RESET}\n" "$1"
    echo -e "${CYAN}${BOLD}╚══════════════════════════════════════════════════════════╝${RESET}"
    echo ""
}

step()    { echo -e "${BLUE}  ▶  ${RESET}${BOLD}$*${RESET}"; }
ok()      { echo -e "${GREEN}  ✔  $*${RESET}"; }
warn()    { echo -e "${YELLOW}  ⚠  $*${RESET}"; }
fail()    { echo -e "${RED}  ✖  $*${RESET}"; }
info()    { echo -e "${DIM}     $*${RESET}"; }

confirm() {
    local prompt="$1"
    local default="${2:-y}"
    local answer
    local hint
    [[ "$default" == "y" ]] && hint="[Y/n]" || hint="[y/N]"
    echo ""
    read -rp "$(echo -e "${YELLOW}  ?  ${prompt} ${hint}: ${RESET}")" answer
    answer="${answer:-$default}"
    [[ "${answer,,}" == "y" ]]
}

spinner() {
    local pid=$1
    local msg=$2
    local spin=('⠋' '⠙' '⠹' '⠸' '⠼' '⠴' '⠦' '⠧' '⠇' '⠏')
    local i=0
    while kill -0 "$pid" 2>/dev/null; do
        printf "\r${CYAN}  %s  %s${RESET}" "${spin[$i]}" "$msg"
        i=$(( (i + 1) % 10 ))
        sleep 0.1
    done
    printf "\r"
}

# ── Root check ────────────────────────────────────────────────────────────────
if [[ $EUID -ne 0 ]]; then
    fail "This script must be run as root."
    info "Please run: sudo ./iscsi_install.sh"
    exit 1
fi

# ── Detect package manager ────────────────────────────────────────────────────
if command -v apt-get &>/dev/null; then
    PKG_MGR="apt-get"
elif command -v dnf &>/dev/null; then
    PKG_MGR="dnf"
elif command -v yum &>/dev/null; then
    PKG_MGR="yum"
else
    fail "No supported package manager found (apt-get / dnf / yum)."
    exit 1
fi

# ── Banner ────────────────────────────────────────────────────────────────────
clear
banner "iSCSI Target Engine Installer  v1.0"

echo -e "${DIM}  Detected package manager : ${PKG_MGR}${RESET}"
echo -e "${DIM}  Hostname                 : $(hostname)${RESET}"
echo -e "${DIM}  Running as               : $(id -un) (UID: $EUID)${RESET}"
echo ""
echo -e "  This script will install ${CYAN}${BOLD}targetcli-fb${RESET} and"
echo -e "  enable the ${CYAN}${BOLD}target${RESET} kernel service on this machine."
echo ""

if ! confirm "Proceed with iSCSI Target installation?"; then
    info "Installation cancelled. No changes have been made."
    exit 0
fi

echo ""

# ── Step 1: Update ────────────────────────────────────────────────────────────
step "[1/3] Refreshing package index..."

if [[ "$PKG_MGR" == "apt-get" ]]; then
    apt-get update -y &>/tmp/iscsi_update.log &
else
    $PKG_MGR makecache -y &>/tmp/iscsi_update.log &
fi
BGPID=$!
spinner $BGPID "Updating package cache..."

if wait $BGPID; then
    ok "Package index updated successfully."
else
    warn "Package index update returned a warning — continuing anyway."
    info "Check /tmp/iscsi_update.log for details."
fi

echo ""

# ── Step 2: Install targetcli-fb ─────────────────────────────────────────────
step "[2/3] Installing targetcli-fb..."

if command -v targetcli &>/dev/null; then
    ok "targetcli is already installed on this system."
    info "Version: $(targetcli version 2>/dev/null || echo 'unknown')"
else
    $PKG_MGR install -y targetcli-fb &>/tmp/iscsi_install.log &
    BGPID=$!
    spinner $BGPID "Downloading and installing targetcli-fb..."

    if wait $BGPID; then
        ok "targetcli-fb installed successfully."
    else
        fail "Package installation failed."
        echo ""
        echo -e "${RED}  Last 10 lines of install log:${RESET}"
        tail -10 /tmp/iscsi_install.log
        echo ""
        fail "Cannot continue without targetcli-fb."
        exit 1
    fi
fi

echo ""

# ── Step 3: Enable & start target service ────────────────────────────────────
step "[3/3] Enabling and starting the 'target' service..."

# Enable service at boot
if systemctl enable target &>/dev/null; then
    info "Service 'target' enabled at startup."
else
    warn "Could not enable 'target' service at startup. It may not exist yet."
fi

# Start / restart service
if systemctl restart target &>/dev/null; then
    ok "Service 'target' started successfully."
else
    warn "Service 'target' failed to restart. Trying alternative service names..."

    if systemctl restart rtslib-fb-targetctl &>/dev/null; then
        ok "Service 'rtslib-fb-targetctl' started successfully."
    elif systemctl restart targetcli &>/dev/null; then
        ok "Service 'targetcli' started successfully."
    else
        fail "All attempts to start an iSCSI target service failed."
        echo ""
        info "Troubleshooting tips:"
        info "  • Check: systemctl list-units | grep target"
        info "  • Verify: systemctl status target"
        info "  • Review: journalctl -u target -n 30"
        exit 1
    fi
fi

# Verify it is truly running
if systemctl is-active --quiet target \
    || systemctl is-active --quiet rtslib-fb-targetctl \
    || systemctl is-active --quiet targetcli; then
    ok "iSCSI target service is running and healthy."
else
    warn "Service may not be fully active. Please verify manually."
fi

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}${BOLD}╔══════════════════════════════════════════════════════════╗${RESET}"
echo -e "${GREEN}${BOLD}║                                                          ║${RESET}"
echo -e "${GREEN}${BOLD}║   ✔  SUCCESS — iSCSI Target Engine is ready!             ║${RESET}"
echo -e "${GREEN}${BOLD}║                                                          ║${RESET}"
echo -e "${GREEN}${BOLD}║   Next steps:                                            ║${RESET}"
echo -e "${GREEN}${BOLD}║   • Configure targets:  sudo targetcli                   ║${RESET}"
echo -e "${GREEN}${BOLD}║   • Or run the Rust orchestrator for one-shot setup      ║${RESET}"
echo -e "${GREEN}${BOLD}║                                                          ║${RESET}"
echo -e "${GREEN}${BOLD}╚══════════════════════════════════════════════════════════╝${RESET}"
echo ""