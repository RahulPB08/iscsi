#!/bin/bash
# =============================================================================
#  build_lustre.sh — Automated Lustre Kernel Module Builder (Wiki Compliant)
#  Updated to strictly match the Lustre Wiki Virtualbox/Rocky 8.9 Guide
# =============================================================================

BOLD='\033[1m'; RESET='\033[0m'; CYAN='\033[1;36m'
GREEN='\033[1;32m'; YELLOW='\033[1;33m'; RED='\033[1;31m'
BLUE='\033[1;34m'; DIM='\033[2m'

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

step()   { echo -e "${CYAN}  ▶  ${RESET}${BOLD}$*${RESET}"; }
ok()     { echo -e "${GREEN}  ✔  $*${RESET}"; }
warn()   { echo -e "${YELLOW}  ⚠  $*${RESET}"; }
fail()   { echo -e "${RED}  ✖  $*${RESET}"; }
info()   { echo -e "${DIM}     $*${RESET}"; }

confirm() {
    local prompt="$1"
    local default="${2:-y}"
    local answer hint
    [[ "$default" == "y" ]] && hint="[Y/n]" || hint="[y/N]"
    read -rp "$(echo -e "${YELLOW}  ?  ${prompt} ${hint}: ${RESET}")" answer
    answer="${answer:-$default}"
    [[ "${answer,,}" == "y" ]]
}

# Ensure script is run as root
if [[ $EUID -ne 0 ]]; then
    fail "This script must be run as root."
    info "Please run: sudo ./build_lustre.sh"
    exit 1
fi

clear
banner "Lustre Wiki VirtualBox Builder v3.0"

if ! confirm "Ready to begin the Wiki-compliant setup?"; then
    info "Build cancelled."
    exit 0
fi

# ── Phase 1: Dependencies ─────────────────────────────────────────────────────
phase 1 "Installing Build Tools & Development Libraries"

step "Installing pre-requisite software tools..."
if dnf install -y git libtool flex bison wget 2>&1 | tail -3; then
    ok "Core development tools installed."
else
    fail "dnf install failed for core tools."
    exit 1
fi

step "Installing required development libraries from 'devel' repository..."
if dnf --enablerepo=devel install -y libmount-devel libyaml-devel libnl3-devel e2fsprogs-devel 2>&1 | tail -3; then
    ok "Development libraries installed."
else
    fail "dnf install failed for development libraries."
    exit 1
fi

# ── Phase 2: Kernel Packages ──────────────────────────────────────────────────
phase 2 "Installing Lustre-Patched Kernel Packages"

# Specific kernel version defined by the Wiki tutorial matrix
WIKI_KERNEL_VER="4.18.0-513.18.1.el8_lustre.x86_64"
JENKINS_BASE_URL="https://build.whamcloud.com/job/lustre-master/arch=x86_64,build_type=server,distro=el8.9,ib_stack=inkernel/lastSuccessfulBuild/artifact/artifacts/RPMS/x86_64"

step "Installing specific Lustre-LDISKFS targeted kernel distribution packages..."
if dnf install -y \
    "${JENKINS_BASE_URL}/kernel-${WIKI_KERNEL_VER}.rpm" \
    "${JENKINS_BASE_URL}/kernel-core-${WIKI_KERNEL_VER}.rpm" \
    "${JENKINS_BASE_URL}/kernel-devel-${WIKI_KERNEL_VER}.rpm" \
    "${JENKINS_BASE_URL}/kernel-headers-${WIKI_KERNEL_VER}.rpm" \
    "${JENKINS_BASE_URL}/kernel-modules-${WIKI_KERNEL_VER}.rpm" \
    "${JENKINS_BASE_URL}/kernel-modules-internal-${WIKI_KERNEL_VER}.rpm" 2>&1 | tail -5; then
    ok "Lustre-patched kernel packages successfully registered."
else
    fail "Failed to resolve or install targeted kernel packages from Whamcloud Jenkins artifacts."
    exit 1
fi

# ── Phase 3: e2fsprogs repo ───────────────────────────────────────────────────
phase 3 "Configuring Lustre e2fsprogs Repository"

step "Writing repository definitions directly onto /etc/dnf/dnf.conf..."
if ! grep -q "Lustre-e2fsprogs" /etc/dnf/dnf.conf; then
    tee -a /etc/dnf/dnf.conf > /dev/null << 'EOF'

[Lustre-e2fsprogs]
name=Lustre-e2fsprogs
baseurl=http://downloads.whamcloud.com/public/e2fsprogs/latest/el$releasever/
gpgcheck=0
enabled=1
EOF
    ok "Lustre-e2fsprogs repository tracking appended to dnf.conf."
else
    ok "Lustre-e2fsprogs configuration block already active."
fi

step "Upgrading e2fsprogs binaries to modified Lustre version..."
if dnf update -y e2fsprogs 2>&1 | tail -3; then
    ok "e2fsprogs patched successfully."
else
    fail "e2fsprogs patching step failed."
    exit 1
fi

# ── Phase 4: Clone & Compile ──────────────────────────────────────────────────
phase 4 "Cloning & Compiling Lustre from Source"

LUSTRE_SRC="/usr/src/lustre-head"

step "Setting up source destination at $LUSTRE_SRC..."
mkdir -p "$LUSTRE_SRC"

if [[ -d "$LUSTRE_SRC/.git" ]]; then
    warn "Lustre source tracking database already active."
    git -C "$LUSTRE_SRC" pull
else
    # Utilizing public HTTPS fallback structure since SSH profile requires keys setup
    if git clone https://github.com/lustre/lustre-release.git "$LUSTRE_SRC"; then
        ok "Lustre repository successfully pulled down."
    else
        fail "Failed to mirror remote git repository source maps."
        exit 1
    fi
fi

cd "$LUSTRE_SRC" || exit 1

step "Executing configuration layout builds (autogen.sh)..."
if sh autogen.sh &>/dev/null; then
    ok "autogen system pass completed."
else
    fail "Autogen evaluation pipeline broke."
    exit 1
fi

step "Running framework configurations..."
if ./configure 2>&1 | tail -5; then
    ok "Configure system checks cleared."
else
    fail "Configuration verification failed."
    exit 1
fi

step "Compiling core engine targets..."
if make; then
    ok "Compilation complete."
else
    fail "Lustre engine compilation failed."
    exit 1
fi

step "Installing built targets across operating system subsystems..."
if make install; then
    ok "Lustre system software layout deployed successfully."
else
    fail "Target runtime system deployment operations failed."
    exit 1
fi

# ── Complete ──────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}${BOLD}╔══════════════════════════════════════════════════════════╗${RESET}"
echo -e "${GREEN}${BOLD}║    ✔  BUILD COMPLETE — Lustre is installed!               ║${RESET}"
echo -e "${GREEN}${BOLD}╚══════════════════════════════════════════════════════════╝${RESET}"
echo ""

warn "The system will reboot in 5 seconds to load the new kernel..."
sleep 5
reboot
