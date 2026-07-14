#!/usr/bin/env bash
set -e

echo "========================================="
echo "    DataSpringFlow (DSF) Setup Wizard"
echo "========================================="

# Identify current user privileges and installation mode
IS_SUDO=0
if [ "$EUID" -eq 0 ]; then
  IS_SUDO=1
  echo "[Mode] root/sudo privileges detected -> Executing System-wide Global Installation"
else
  echo "[Mode] Normal user privileges detected -> Executing User-level Local Installation"
fi

# Prepare temporary extraction directory
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

# Locate self-extracting boundary and extract tar.gz data
MATCH=$(grep -a -n '^__ARCHIVE_BELOW__$' "$0" | cut -d ':' -f 1)
PAYLOAD_LINE=$((MATCH + 1))

tail -n +"${PAYLOAD_LINE}" "$0" | tar -xzf - -C "$TMP_DIR"

#  Execute differential installation logic based on the mode
if [ "$IS_SUDO" -eq 1 ]; then
  # Global Installation
  BIN_DEST="/usr/local/bin/dsf"
  DATA_DIR="/var/lib/dataspringflow"
  PY_DIR="${DATA_DIR}/py"

  echo "--> 1. Copying binary file to system path..."
  cp "${TMP_DIR}/dsf" "$BIN_DEST"
  chmod +x "$BIN_DEST"

  echo "--> 2. Executing global service initialization (sudo dsf init --global)..."
  # Invoke your Rust CLI to establish global directories and internal database
  dsf init --global

  echo "--> 3. Deploying and sharing global Python Wheel packages..."
  mkdir -p "$PY_DIR"
  cp ${TMP_DIR}/*.whl "$PY_DIR/"

  echo "--> 4. Configuring read permissions for global data directory and shared packages..."
  chmod -R 755 "$DATA_DIR"

  echo ""
  echo "========================================="
  echo "    DSF System-wide Installation Complete!"
  echo "========================================="
  echo "• Administrator CLI Path: $BIN_DEST"
  echo "• Shared Wheel Repository: $PY_DIR"
  echo "• User offline local installation command:"
  echo "  pip install ${PY_DIR}/dataspringflow-*.whl"

else
  #  Personal User Installation
  # Follow XDG spec, prioritize env variables, fallback to ~/.local/share
  XDG_DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
  USER_BIN_DIR="$HOME/.local/bin"
  BIN_DEST="${USER_BIN_DIR}/dsf"
  DATA_DIR="${XDG_DATA_HOME}/dataspringflow"
  PY_DIR="${DATA_DIR}/py"

  echo "--> 1. Copying binary file to user-level path..."
  mkdir -p "$USER_BIN_DIR"
  cp "${TMP_DIR}/dsf" "$BIN_DEST"
  chmod +x "$BIN_DEST"

  # Prompt normal user to check their PATH environment variable
  if [[ ":$PATH:" != *":$USER_BIN_DIR:"* ]]; then
    echo "Warning: $USER_BIN_DIR is not in your PATH environment variable."
    echo "         Please append it to your ~/.bashrc or ~/.config/fish/config.fish later."
  fi

  echo "--> 2. Executing user-level service initialization (dsf init)..."
  # Invoke your Rust CLI to establish private configuration and database based on ProjectDirs
  "$BIN_DEST" init

  echo "--> 3. Deploying private Python Wheel packages..."
  mkdir -p "$PY_DIR"
  cp ${TMP_DIR}/*.whl "$PY_DIR/"
  chmod -R 700 "$DATA_DIR" # Keep personal data isolated and private

  echo ""
  echo "========================================="
  echo "    DSF User-level Installation Complete!"
  echo "========================================="
  echo "• Personal CLI Path: $BIN_DEST"
  echo "• Local Wheel Backup Path: $PY_DIR"
  echo "• Offline local installation command:"
  echo "  pip install ${PY_DIR}/dataspringflow-*.whl"
fi

exit 0

__ARCHIVE_BELOW__
