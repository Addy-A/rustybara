#!/usr/bin/env bash
# install.sh — Install rbara into PREFIX (default: ~/.local).
#
# Usage:
#   ./install.sh                       # installs to ~/.local
#   PREFIX=/usr/local ./install.sh     # system-wide (needs sudo)
#
# Layout after install:
#   $PREFIX/bin/rbara               wrapper script (this is what you run)
#   $PREFIX/lib/rbara/rbara-bin     the actual binary
#   $PREFIX/lib/rbara/libpdfium.dylib

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"

echo "==> Installing rbara to $PREFIX"

# Strip macOS quarantine flag (set by Gatekeeper when downloaded via browser).
# Safe to run even if the attribute isn't present.
xattr -dr com.apple.quarantine "$SCRIPT_DIR" 2>/dev/null || true

mkdir -p "$PREFIX/bin" "$PREFIX/lib/rbara"

install -m 0755 "$SCRIPT_DIR/lib/rbara/rbara-bin"        "$PREFIX/lib/rbara/rbara-bin"
install -m 0755 "$SCRIPT_DIR/lib/rbara/libpdfium.dylib"  "$PREFIX/lib/rbara/libpdfium.dylib"
install -m 0755 "$SCRIPT_DIR/bin/rbara"                  "$PREFIX/bin/rbara"

echo
echo "[OK] Installed."
echo "     Wrapper:  $PREFIX/bin/rbara"
echo "     Library:  $PREFIX/lib/rbara/"
echo

# PATH — auto-configure when the bin dir isn't on PATH yet
case ":$PATH:" in
  *":$PREFIX/bin:"*)
    echo "     '$PREFIX/bin' is already on your PATH."
    ;;
  *)
    # Determine which shell rc to write to
    RC_FILE="$HOME/.zshrc"
    if [[ "$SHELL" != *zsh* && -f "$HOME/.bash_profile" ]]; then
      RC_FILE="$HOME/.bash_profile"
    elif [[ "$SHELL" != *zsh* && -f "$HOME/.bashrc" ]]; then
      RC_FILE="$HOME/.bashrc"
    fi

    EXPORT_LINE="export PATH=\"$PREFIX/bin:\$PATH\""

    if grep -qxF "$EXPORT_LINE" "$RC_FILE" 2>/dev/null; then
      echo "     '$PREFIX/bin' is already configured in $RC_FILE."
    else
      printf '\n# rbara — added by installer\n%s\n' "$EXPORT_LINE" >> "$RC_FILE"
      echo "     Added '$PREFIX/bin' to PATH in $RC_FILE."
      echo "     Run 'source $RC_FILE' or open a new terminal to apply."
    fi
    ;;
esac

echo "     Run:    rbara --help"
echo "     Uninstall: PREFIX=$PREFIX $SCRIPT_DIR/uninstall.sh"
