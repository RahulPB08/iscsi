#!/bin/bash
# =============================================================================
#  build_lustre.sh — Lustre Kernel Module Builder (Repository Edition)
#  Compiles and installs Lustre from source on RHEL 8 / Rocky Linux 8.
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
    local answer hint
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

clear
banner "Lustre Kernel Module Builder  v1.2"

if ! confirm "Ready to begin? This requires an active internet connection"; then
    info "Build cancelled."
    exit 0
fi

# ── Phase 1: Build tools ──────────────────────────────────────────────────────
phase 1 "Installing Build Tools & Development Libraries"

step "Installing core development tools..."
if dnf install -y git libtool flex bison wget 2>&1 | tail -3; then
    ok "Core development tools installed."
else
    fail "dnf install failed for core tools."
    exit 1
fi

step "Installing required development libraries..."
if dnf --enablerepo=devel install -y libmount-devel libyaml-devel libnl3-devel e2fsprogs-devel 2>&1 | tail -3; then
    ok "Development libraries installed."
else
    fail "dnf install failed for development libraries."
    exit 1
fi

# ── Phase 2: Configure Repos & Install Kernel ─────────────────────────────────
phase 2 "Configuring Repositories & Installing Lustre Kernel"

step "Adding Whamcloud Lustre Server repository..."
cat << 'EOF' > /etc/yum.repos.d/lustre-server.repo
[lustre-server]
name=Lustre Server Stable
baseurl=https://downloads.whamcloud.com/public/lustre/latest-release/el8/server/RPMS/x86_64/
gpgcheck=0
enabled=1
EOF

step "Adding EPEL repository for system dependencies..."
dnf install -y epel-release &>/dev/null

step "Installing Lustre-patched kernel packages via repository..."
if dnf install -y --enablerepo=lustre-server \
    kernel \
    kernel-core \
    kernel-devel \
    kernel-headers \
    kernel-modules \
    kernel-modules-internal \
    p7zip quilt; then
    ok "Lustre-patched kernel packages successfully installed."
else
    fail "Repository installation failed."
    exit 1
fi

# ── Phase 3: e2fsprogs repo ───────────────────────────────────────────────────
phase 3 "Configuring Lustre e2fsprogs Repository"

step "Configuring Lustre-e2fsprogs repository..."
cat << 'EOF' > /etc/yum.repos.d/lustre-e2fsprogs.repo
[lustre-e2fsprogs]
name=Lustre-e2fsprogs
baseurl=http://downloads.whamcloud.com/public/e2fsprogs/latest/el8/
gpgcheck=0
enabled=1
EOF

step "Upgrading e2fsprogs to Lustre version..."
if dnf upgrade -y e2fsprogs 2>&1 | tail -3; then
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
        git -C "$LUSTRE_SRC" pull && ok "Repository updated." || warn "git pull failed."
    fi
else
    step "Cloning Lustre repository to $LUSTRE_SRC..."
    mkdir -p "$LUSTRE_SRC"
    if git clone https://github.com/lustre/lustre-release.git "$LUSTRE_SRC"; then
        ok "Lustre repository cloned via HTTPS."
    else
        fail "git clone failed."
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

# Dynamically locate the newly installed Lustre kernel header path
LUSTRE_KERNEL_DIR=$(ls -d /usr/src/kernels/*_lustre* | head -n 1)

step "Running ./configure using directory: ${LUSTRE_KERNEL_DIR}..."
if ./configure --with-linux="${LUSTRE_KERNEL_DIR}" 2>&1 | tail -5; then
    ok "configure completed."
else
    fail "configure failed."
    exit 1
fi

CPUS=$(nproc)
step "Compiling Lustre with ${CPUS} CPU cores..."
echo -e "${DIM}     Follow progress with: tail -f /tmp/lustre_build.log${RESET}"
if make -j"${CPUS}" > /tmp/lustre_build.log 2>&1; then
    ok "Compilation complete."
else
    fail "make failed. Review /tmp/lustre_build.log for details."
    tail -20 /tmp/lustre_build.log
    exit 1
fi

step "Installing Lustre modules and utilities..."
if make install > /tmp/lustre_install.log 2>&1; then
    ok "Lustre installed successfully."
else
    fail "make install failed."
    exit 1
fi

# ── Complete ──────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}${BOLD}╔══════════════════════════════════════════════════════════╗${RESET}"
echo -e "${GREEN}${BOLD}║   ✔  BUILD COMPLETE — Lustre is installed!               ║${RESET}"
echo -e "${GREEN}${BOLD}╚══════════════════════════════════════════════════════════╝${RESET}"
echo ""

warn "The system will reboot in 5 seconds to load the new kernel..."
sleep 5
reboot
