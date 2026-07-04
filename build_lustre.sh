#!/bin/bash
# =============================================================================
#  build_lustre.sh — Automated Lustre Kernel Module Builder (Wiki Compliant)
#  Based on the Official Lustre Wiki Documentation for Rocky Linux 8
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

rm -f /etc/yum.repos.d/lustre-*.repo

clear
banner "Lustre Kernel Module Builder  v2.1"

if ! confirm "Ready to begin? This requires an active internet connection"; then
    info "Build cancelled."
    exit 0
fi

# ── Phase 1: Build tools ──────────────────────────────────────────────────────
phase 1 "Installing Build Tools & Development Libraries"

step "Resetting system DNF index allocations..."
dnf clean all &>/dev/null

step "Installing core development tools (git, libtool, flex, bison, wget, curl)..."
if dnf install -y git libtool flex bison wget curl 2>&1 | tail -3; then
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

# ── Phase 2: Lustre kernel packages ──────────────────────────────────────────
phase 2 "Downloading & Processing Lustre-Patched Kernel Packages"

RPM_DIR="/tmp/lustre_rpms"
mkdir -p "$RPM_DIR"
rm -f "$RPM_DIR"/*.rpm

# Detect pathing by testing server availability dynamically
EPEL_URL="https://dl.fedoraproject.org/pub/epel/8/Everything/x86_64/Packages"

if curl --output /dev/null --silent --head --fail "https://downloads.whamcloud.com/public/lustre/lustre-2.15.5/el8.9/server/RPMS/x86_64/kernel-headers-4.18.0-513.18.1.el8_lustre.x86_64.rpm"; then
    WHAM_URL="https://downloads.whamcloud.com/public/lustre/lustre-2.15.5/el8.9/server/RPMS/x86_64"
    K_VERSION="4.18.0-513.18.1.el8_lustre.x86_64"
else
    # Universal fallback stable vault endpoint
    WHAM_URL="https://downloads.whamcloud.com/public/lustre/lustre-2.15.5/el8/server/RPMS/x86_64"
    K_VERSION="4.18.0-477.27.1.el8_lustre.x86_64"
fi

info "Selected mirror source: ${WHAM_URL}"
info "Selected kernel target: ${K_VERSION}"

declare -a KERNEL_PACKAGES=(
    "kernel-${K_VERSION}.rpm"
    "kernel-core-${K_VERSION}.rpm"
    "kernel-devel-${K_VERSION}.rpm"
    "kernel-headers-${K_VERSION}.rpm"
    "kernel-modules-${K_VERSION}.rpm"
    "kernel-modules-internal-${K_VERSION}.rpm"
)

step "Downloading explicit kernel packages with network recovery..."
for package in "${KERNEL_PACKAGES[@]}"; do
    info "Fetching ${package}..."
    if ! curl -L --fail --retry 5 --connect-timeout 30 -o "$RPM_DIR/$package" "${WHAM_URL}/${package}"; then
        fail "Failed download stream for ${package}. Mirror path structures may have drifted."
        exit 1
    fi
done

step "Downloading supplemental system testing tools..."
curl -L --fail --retry 5 --connect-timeout 30 -o "$RPM_DIR/p7zip.rpm" "${EPEL_URL}/p/p7zip-16.02-20.el8.x86_64.rpm" || \
    curl -L --fail -o "$RPM_DIR/p7zip.rpm" "https://downloads.datastax.com/enterprise/p7zip-16.02-20.el8.x86_64.rpm"

curl -L --fail --retry 5 --connect-timeout 30 -o "$RPM_DIR/quilt.rpm" "${EPEL_URL}/q/quilt-0.66-2.el8.noarch.rpm" || \
    curl -L --fail -o "$RPM_DIR/quilt.rpm" "https://vault.centos.org/centos/8/AppStream/x86_64/os/Packages/quilt-0.66-2.el8.noarch.rpm"

step "Injecting packages directly via RPM subsystem..."
if rpm -ivh --nodeps --force "$RPM_DIR"/*.rpm; then
    ok "Lustre-patched kernel packages successfully installed."
else
    fail "RPM module installation transaction failed."
    exit 1
fi

# ── Phase 3: e2fsprogs repo ───────────────────────────────────────────────────
phase 3 "Configuring Lustre e2fsprogs Repository"

step "Writing repository definitions directly onto /etc/dnf/dnf.conf..."
if ! grep -q "Lustre-e2fsprogs" /etc/dnf/dnf.conf; then
    tee -a /etc/dnf/dnf.conf > /dev/null << 'EOF'

[Lustre-e2fsprogs]
name=Lustre-e2fsprogs
baseurl=http://downloads.whamcloud.com/public/e2fsprogs/latest/el8/
gpgcheck=0
enabled=1
EOF
    ok "Lustre-e2fsprogs repository appended successfully."
else
    ok "Lustre-e2fsprogs repository tracking already active."
fi

step "Upgrading e2fsprogs tools to modified Lustre version..."
if dnf update -y --disablerepo=* --enablerepo=baseos,appstream,Lustre-e2fsprogs e2fsprogs 2>&1 | tail -3; then
    ok "e2fsprogs updated successfully."
else
    fail "e2fsprogs update target failed."
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
        fail "git clone failed via HTTPS connection profile."
        exit 1
    fi
fi

step "Running autogen.sh..."
cd "$LUSTRE_SRC" || exit 1
if sh autogen.sh &>/dev/null; then
    ok "autogen.sh completed."
else
    fail "autogen.sh execution failed."
    exit 1
fi

LUSTRE_KERNEL_DIR="/usr/src/kernels/${K_VERSION}"

step "Running ./configure linking headers at: ${LUSTRE_KERNEL_DIR}..."
if ./configure --with-linux="${LUSTRE_KERNEL_DIR}" 2>&1 | tail -5; then
    ok "Configure step passed."
else
    fail "Configure framework validation failed."
    exit 1
fi

CPUS=$(nproc)
step "Compiling Lustre engine using ${CPUS} CPU threads..."
if make -j"${CPUS}" > /tmp/lustre_build.log 2>&1; then
    ok "Compilation complete."
else
    fail "make build processing failed. Review /tmp/lustre_build.log for details."
    tail -20 /tmp/lustre_build.log
    exit 1
fi

step "Installing Lustre modules and utilities..."
if make install > /tmp/lustre_install.log 2>&1; then
    ok "Lustre system software layout installed successfully."
else
    fail "make install target deployment failed."
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
