#!/bin/bash
# =============================================================================
#  build_lustre.sh — Lustre Kernel Module Builder (Complete & Fixed)
#  Compiles and installs Lustre from source on RHEL 8 / Rocky Linux 8.
#  Run this on the MGS/MDS and OSS nodes BEFORE running the Rust orchestrator.
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

phase() {
    echo ""
    echo -e "${BLUE}${BOLD}┌─────────────────────────────────────────────────┐${RESET}"
    printf "${BLUE}${BOLD}│  %-47s│${RESET}\n" "Phase $1 — $2"
    echo -e "${BLUE}${BOLD}└─────────────────────────────────────────────────┘${RESET}"
    echo ""
}

step()   { echo -e "${CYAN}  ▶  $*${RESET}"; }
ok()     { echo -e "${GREEN}  ✔  $*${RESET}"; }
warn()   { echo -e "${YELLOW}  ⚠  $*${RESET}"; }
fail()   { echo -e "${RED}  ✖  $*${RESET}"; }
info()   { echo -e "${DIM}     $*${RESET}"; }

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

# ── Root check ────────────────────────────────────────────────────────────────
if [[ $EUID -ne 0 ]]; then
    fail "This script must be run as root."
    info "Please run: sudo ./build_lustre.sh"
    exit 1
fi

# ── Banner ────────────────────────────────────────────────────────────────────
clear
banner "Lustre Kernel Module Builder  v1.1"

echo -e "${DIM}  This script will:${RESET}"
echo -e "${DIM}  1. Install build tools & dev libraries${RESET}"
echo -e "${DIM}  2. Download Lustre-patched kernel packages${RESET}"
echo -e "${DIM}  3. Configure the Lustre e2fsprogs repository${RESET}"
echo -e "${DIM}  4. Clone and compile the Lustre source tree${RESET}"
echo -e "${DIM}  5. Install Lustre and reboot the system${RESET}"
echo ""

warn "Build time: 10–30 minutes depending on CPU cores."
warn "The system will REBOOT automatically after a successful install."
echo ""

if ! confirm "Ready to begin? This requires an active internet connection"; then
    info "Build cancelled. No changes have been made."
    exit 0
fi

# ── Phase 1: Build tools ──────────────────────────────────────────────────────
phase 1 "Installing Build Tools & Development Libraries"

step "Installing core development tools (git, libtool, flex, bison, wget)..."
if dnf install -y git libtool flex bison wget 2>&1 | tail -3; then
    ok "Core development tools installed."
else
    fail "dnf install failed for core tools."
    exit 1
fi

step "Installing required development libraries..."
if dnf --enablerepo=devel install -y \
        libmount-devel libyaml-devel libnl3-devel e2fsprogs-devel 2>&1 | tail -3; then
    ok "Development libraries installed."
else
    fail "dnf install failed for development libraries."
    exit 1
fi

# ── Phase 2: Lustre kernel packages ──────────────────────────────────────────
phase 2 "Downloading Lustre-Patched Kernel Packages"

# Stable vault release URL matching EL8 minor version targets
WHAM="https://downloads.whamcloud.com/public/lustre/lustre-2.15.5/el8.8/server/RPMS/x86_64"
EPEL="https://dl.fedoraproject.org/pub/epel/8/Everything/x86_64/Packages"
KERN_VER="4.18.0-477.27.1.el8_lustre.x86_64"

step "Fetching packages from Whamcloud & EPEL vaults..."
if dnf install -y \
    "${WHAM}/kernel-${KERN_VER}.rpm" \
    "${WHAM}/kernel-core-${KERN_VER}.rpm" \
    "${WHAM}/kernel-devel-${KERN_VER}.rpm" \
    "${WHAM}/kernel-headers-${KERN_VER}.rpm" \
    "${WHAM}/kernel-modules-${KERN_VER}.rpm" \
    "${WHAM}/kernel-modules-internal-${KERN_VER}.rpm" \
    "${EPEL}/p/p7zip-16.02-20.el8.x86_64.rpm" \
    "${EPEL}/q/quilt-0.66-2.el8.noarch.rpm"; then
    ok "Lustre-patched kernel packages installed."
else
    fail "Package download/install failed."
    warn "Verify your networking or check path updates on downloads.whamcloud.com"
    exit 1
fi

# ── Phase 3: e2fsprogs repo ───────────────────────────────────────────────────
phase 3 "Configuring Lustre e2fsprogs Repository"

step "Appending Lustre-e2fsprogs repo to /etc/dnf/dnf.conf..."
if ! grep -q "Lustre-e2fsprogs" /etc/etc/dnf/dnf.conf 2>/dev/null && ! grep -q "Lustre-e2fsprogs" /etc/dnf/dnf.conf; then
    tee -a /etc/dnf/dnf.conf > /dev/null << 'EOF'

[Lustre-e2fsprogs]
name=Lustre-e2fsprogs
baseurl=http://downloads.whamcloud.com/public/e2fsprogs/latest/el$releasever/
gpgcheck=0
enabled=1
EOF
    ok "Lustre-e2fsprogs repository added."
else
    ok "Lustre-e2fsprogs repository already configured. Skipping."
fi

step "Upgrading e2fsprogs to Lustre-patched version..."
if dnf update -y e2fsprogs 2>&1 | tail -3; then
    ok "e2fsprogs updated."
else
    fail "e2fsprogs update failed."
    exit 1
fi

# ── Phase 4: Clone & Compile ──────────────────────────────────────────────────
phase 4 "Cloning & Compiling Lustre from Source"

LUSTRE_SRC="/usr/src/lustre-head"

if [[ -d "$LUSTRE_SRC/.git" ]]; then
    warn "Lustre source directory already exists at $LUSTRE_SRC."
    if confirm "Pull latest changes instead of re-cloning?"; then
        step "Updating existing Lustre repository..."
        git -C "$LUSTRE_SRC" pull && ok "Repository updated." || warn "git pull failed — continuing with existing source."
    fi
else
    step "Cloning Lustre repository to $LUSTRE_SRC..."
    mkdir -p "$LUSTRE_SRC"
    chmod 777 "$LUSTRE_SRC"
    # Using public HTTPS instead of SSH key protocol to bypass sudo permission blocks
    if git clone https://github.com/lustre/lustre-release.git "$LUSTRE_SRC"; then
        ok "Lustre repository cloned."
    else
        fail "git clone failed via HTTPS connection."
        exit 1
    fi
fi

step "Running autogen.sh..."
cd "$LUSTRE_SRC" || exit 1
if sh autogen.sh &>/dev/null; then
    ok "autogen.sh completed."
else
    fail "autogen.sh failed."
    exit 1
fi

step "Running ./configure targeting Lustre kernel headers..."
if ./configure --with-linux=/usr/src/kernels/${KERN_VER} 2>&1 | tail -5; then
    ok "configure completed."
else
    fail "configure failed. Check configuration flags or dependencies above."
    exit 1
fi

CPUS=$(nproc)
step "Compiling Lustre with ${CPUS} CPU cores (this takes 10–30 minutes)..."
echo -e "${DIM}     Follow progress with: tail -f /tmp/lustre_build.log${RESET}"
if make -j"${CPUS}" > /tmp/lustre_build.log 2>&1; then
    ok "Compilation complete."
else
    fail "make failed. Review /tmp/lustre_build.log for details."
    echo ""
    tail -20 /tmp/lustre_build.log
    exit 1
fi

step "Installing Lustre modules and utilities..."
if make install > /tmp/lustre_install.log 2>&1; then
    ok "Lustre installed successfully."
else
    fail "make install failed. Review /tmp/lustre_install.log."
    exit 1
fi

# ── Complete ──────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}${BOLD}╔══════════════════════════════════════════════════════════╗${RESET}"
echo -e "${GREEN}${BOLD}║                                                          ║${RESET}"
echo -e "${GREEN}${BOLD}║   ✔  BUILD COMPLETE — Lustre is installed!               ║${RESET}"
echo -e "${GREEN}${BOLD}║                                                          ║${RESET}"
echo -e "${GREEN}${BOLD}║   Next step:  Run test_lustre.sh after reboot            ║${RESET}"
echo -e "${GREEN}${BOLD}║   to verify the single-node loopback cluster works.      ║${RESET}"
echo -e "${GREEN}${BOLD}║                                                          ║${RESET}"
echo -e "${GREEN}${BOLD}╚══════════════════════════════════════════════════════════╝${RESET}"
echo ""

warn "The system will reboot in 5 seconds to load the new kernel..."
info "Press Ctrl+C now to cancel the reboot."
sleep 5
reboot
