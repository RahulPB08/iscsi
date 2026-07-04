#!/bin/bash
# =============================================================================
#  rust_install.sh — Rust & Cargo Toolchain Installer
#  Installs Rust via rustup and verifies the installation.
# =============================================================================

BOLD='\033[1m'; RESET='\033[0m'; CYAN='\033[1;36m'
GREEN='\033[1;32m'; YELLOW='\033[1;33m'; RED='\033[1;31m'
BLUE='\033[1;34m'; DIM='\033[2m'

banner() {
    echo ""; echo -e "${CYAN}${BOLD}╔══════════════════════════════════════════════════════╗${RESET}"
    printf "${CYAN}${BOLD}║  %-52s║${RESET}\n" "$1"
    echo -e "${CYAN}${BOLD}╚══════════════════════════════════════════════════════╝${RESET}"; echo ""
}
step() { echo -e "${BLUE}  ▶  ${RESET}${BOLD}$*${RESET}"; }
ok()   { echo -e "${GREEN}  ✔  $*${RESET}"; }
warn() { echo -e "${YELLOW}  ⚠  $*${RESET}"; }
fail() { echo -e "${RED}  ✖  $*${RESET}"; }
info() { echo -e "${DIM}     $*${RESET}"; }
confirm() {
    local answer; local hint; [[ "${2:-y}" == "y" ]] && hint="[Y/n]" || hint="[y/N]"
    read -rp "$(echo -e "${YELLOW}  ?  $1 ${hint}: ${RESET}")" answer
    answer="${answer:-${2:-y}}"; [[ "${answer,,}" == "y" ]]
}

clear
banner "Rust & Cargo Toolchain Installer  v1.0"
echo -e "${DIM}  Hostname: $(hostname)  |  User: $(id -un)${RESET}"; echo ""

if command -v rustc &>/dev/null && command -v cargo &>/dev/null; then
    echo -e "${GREEN}${BOLD}  Rust is already installed!${RESET}"
    echo -e "  ${CYAN}rustc${RESET}  $(rustc --version)"
    echo -e "  ${CYAN}cargo${RESET}  $(cargo --version)"; echo ""
    confirm "Update / reinstall Rust?" || { info "No changes made."; exit 0; }
fi

echo -e "  Installs ${CYAN}${BOLD}Rust stable${RESET} via rustup (https://sh.rustup.rs)"; echo ""
confirm "Proceed with Rust installation?" || { info "Cancelled."; exit 0; }
echo ""

step "[1/3] Installing system build dependencies..."
if command -v dnf &>/dev/null; then
    sudo dnf install -y curl gcc gcc-c++ make 2>&1 | grep -E "(Installing|Already|Error)" | head -5
elif command -v apt-get &>/dev/null; then
    sudo apt-get install -y curl gcc make build-essential 2>&1 | grep -E "(Installing|already|Error)" | head -5
else
    warn "Unknown package manager — skipping dependency install."
fi
command -v curl &>/dev/null && ok "Build dependencies ready." || { fail "curl not found. Please install it and retry."; exit 1; }
echo ""

step "[2/3] Downloading and installing Rust via rustup..."
if curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y; then
    ok "Rust stable toolchain installed."
else
    fail "Rust installation failed. Check your internet connection."; exit 1
fi
echo ""

step "[3/3] Loading Cargo environment..."
[[ -f "$HOME/.cargo/env" ]] && source "$HOME/.cargo/env" && ok "Cargo environment loaded."

RUSTC_VER="$(rustc --version 2>/dev/null)"
CARGO_VER="$(cargo --version 2>/dev/null)"
echo ""

if [[ -n "$RUSTC_VER" && -n "$CARGO_VER" ]]; then
    echo -e "${GREEN}${BOLD}╔══════════════════════════════════════════════════════╗${RESET}"
    echo -e "${GREEN}${BOLD}║   ✔  SUCCESS — Rust is installed and working!        ║${RESET}"
    printf  "${GREEN}${BOLD}║   rustc  %-42s║${RESET}\n" "${RUSTC_VER}"
    printf  "${GREEN}${BOLD}║   cargo  %-42s║${RESET}\n" "${CARGO_VER}"
    echo -e "${GREEN}${BOLD}║                                                      ║${RESET}"
    echo -e "${GREEN}${BOLD}║   Build:  cargo build --release                      ║${RESET}"
    echo -e "${GREEN}${BOLD}║   Run:    sudo ./target/release/iscsi_setup           ║${RESET}"
    echo -e "${GREEN}${BOLD}╚══════════════════════════════════════════════════════╝${RESET}"
else
    fail "Rust not found in PATH after install."
    warn "Open a new terminal or run:  source \"\$HOME/.cargo/env\""
fi
echo ""
