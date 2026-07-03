```bash
#!/bin/bash

set -e

echo "========================================"
echo " Installing Rust and Cargo"
echo "========================================"

# Install required dependencies
sudo dnf install -y curl gcc gcc-c++ make

# Download and install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Load Cargo environment
source "$HOME/.cargo/env"

echo ""
echo "========================================"
echo " Installation Complete"
echo "========================================"

echo "Rust Version:"
rustc --version

echo ""
echo "Cargo Version:"
cargo --version

echo ""
echo "Rust is installed successfully!"
echo ""
echo "If 'cargo' is not available in a new terminal, run:"
echo "source \$HOME/.cargo/env"
```
