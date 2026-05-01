#!/usr/bin/env bash
# install.sh — Install rbara into PREFIX (default: ~/.local).
#
# Usage:
#   ./install.sh                 # installs to ~/.local
#   PREFIX=/usr/local ./install.sh   # system-wide (needs sudo)
#
# Layout after install:
#   $PREFIX/bin/rbara            wrapper script (this is what you run)
#   $PREFIX/lib/rbara/rbara-bin  the actual binary
#   $PREFIX/lib/rbara/libpdfium.so

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"

echo "==> Installing rbara to $PREFIX"

mkdir -p "$PREFIX/bin" "$PREFIX/lib/rbara"

install -m 0755 "$SCRIPT_DIR/lib/rbara/rbara-bin"      "$PREFIX/lib/rbara/rbara-bin"
install -m 0755 "$SCRIPT_DIR/lib/rbara/libpdfium.so"   "$PREFIX/lib/rbara/libpdfium.so"
install -m 0755 "$SCRIPT_DIR/bin/rbara"                "$PREFIX/bin/rbara"

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
    # Pick the right rc file: prefer zshrc if running zsh, else bashrc/bash_profile
    if [[ "$SHELL" == *zsh* ]]; then
      RC_FILE="$HOME/.zshrc"
    elif [[ -f "$HOME/.bash_profile" ]]; then
      RC_FILE="$HOME/.bash_profile"
    else
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
