#!/bin/bash
# Builds Mesh and its helpers in release and installs them for this user
# (no root): mesh, mesh-wa, mesh-dc and mesh-node in ~/.local/bin, Mesh's icon
# and a .desktop entry. The Linux counterpart of build.ps1 + mesh.iss.
#
# mesh-node comes from a Mesh-Node checkout next to this one, or from
# $MESH_NODE; without one, Mesh installs without its own network.
#
#   installer/install-linux.sh              build and install
#   ./install-linux.sh                      in an unpacked release: install it
#   installer/install-linux.sh --uninstall  remove them (keeps accounts and chats)
#
# mesh-dc is only installed once `discord_native_wrapper` is set in
# ~/.config/mesh/config.toml, so Discord never connects from your own IP by
# accident; see README.md.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
bin="$HOME/.local/bin"
apps="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
icons="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/256x256/apps"
config="${XDG_CONFIG_HOME:-$HOME/.config}/mesh/config.toml"

if [[ "${1:-}" == --uninstall ]]; then
  rm -fv "$bin/mesh" "$bin/mesh-wa" "$bin/mesh-dc" "$bin/mesh-node" "$apps/mesh.desktop" "$icons/mesh.png"
  exit 0
fi

here=$(cd "$(dirname "$0")" && pwd)
if [[ -x "$here/mesh" ]]; then
  # An unpacked release download: the binaries sit beside this script.
  app="$here/mesh" wa="$here/mesh-wa" dc="$here/mesh-dc" engine="$here/mesh-node" icon="$here/mesh.png"
else
  (cd "$root" && cargo build --release -p mesh-app)
  (cd "$root/mesh-wa" && cargo build --release)
  (cd "$root/mesh-dc" && cargo build --release)
  node="${MESH_NODE:-$root/../Mesh-Node}"
  engine="$node/target/release/mesh-node"
  if [[ -f "$node/Cargo.toml" ]]; then
    (cd "$node" && cargo build --release -p mesh-node)
  fi
  app="$root/target/release/mesh-app" wa="$root/mesh-wa/target/release/mesh-wa"
  dc="$root/mesh-dc/target/release/mesh-dc" icon="$root/crates/mesh-app/assets/mesh.png"
fi

mkdir -p "$bin" "$apps" "$icons"
install -m755 "$app" "$bin/mesh"
install -m755 "$wa" "$bin/mesh-wa"
if grep -q '^discord_native_wrapper' "$config" 2>/dev/null; then
  install -m755 "$dc" "$bin/mesh-dc"
else
  echo "mesh-dc not installed: set discord_native_wrapper in $config first" >&2
fi
if [[ -x "$engine" ]]; then
  install -m755 "$engine" "$bin/mesh-node"
else
  echo "mesh-node not installed: no Mesh-Node checkout (set MESH_NODE)" >&2
fi
install -m644 "$icon" "$icons/mesh.png"
cat > "$apps/mesh.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Mesh
GenericName=Chat client
Comment=Matrix, WhatsApp and Discord in one lightweight client
Exec=$bin/mesh
Icon=mesh
StartupWMClass=mesh
Terminal=false
Categories=Network;InstantMessaging;Chat;
EOF
update-desktop-database "$apps" 2>/dev/null || true
echo "installed to $bin"
