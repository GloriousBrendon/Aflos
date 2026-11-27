#!/bin/bash
# Builds Aflos and its helpers in release and installs them for this user
# (no root): aflos, aflos-wa, aflos-dc and mesh-node in ~/.local/bin, Aflos's
# icons and a .desktop entry. The Linux counterpart of build.ps1 + aflos.iss.
#
# mesh-node comes from a Mesh-Node checkout next to this one, or from
# $MESH_NODE; without one, Aflos installs without its own network.
#
#   installer/install-linux.sh              build and install
#   ./install-linux.sh                      in an unpacked release: install it
#   installer/install-linux.sh --uninstall  remove them (keeps accounts and chats)
#
# aflos-dc is only installed once `discord_native_wrapper` is set in
# ~/.config/aflos/config.toml (or Mesh's old ~/.config/mesh/config.toml), so
# Discord never connects from your own IP by accident; see README.md.
#
# Upgrading from Mesh (before the rename): its mesh, mesh-wa, mesh-dc, menu
# entry and icon are removed, and ~/.local/bin/mesh is left as a link to
# aflos, which Mesh 0.3.1's updater starts after installing. Aflos copies
# Mesh's accounts and settings on its first start and keeps the old ones.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
bin="$HOME/.local/bin"
share="${XDG_DATA_HOME:-$HOME/.local/share}"
apps="$share/applications"
icons="$share/icons/hicolor"
configs="${XDG_CONFIG_HOME:-$HOME/.config}"
sizes="16 32 48 64 128 256 512"

# Our link from an earlier install (never someone else's ~/.local/bin/mesh).
ours_mesh_link() { [[ -L "$bin/mesh" && "$(readlink "$bin/mesh")" == aflos ]]; }

if [[ "${1:-}" == --uninstall ]]; then
  rm -fv "$bin/aflos" "$bin/aflos-wa" "$bin/aflos-dc" "$bin/mesh-node" "$apps/aflos.desktop" "$icons/scalable/apps/aflos.svg"
  for s in $sizes; do rm -fv "$icons/${s}x${s}/apps/aflos.png"; done
  if ours_mesh_link; then rm -fv "$bin/mesh"; fi
  exit 0
fi

here=$(cd "$(dirname "$0")" && pwd)
if [[ -x "$here/aflos" ]]; then
  # An unpacked release download: the binaries sit beside this script.
  app="$here/aflos" wa="$here/aflos-wa" dc="$here/aflos-dc" engine="$here/mesh-node" hicolor="$here/hicolor"
else
  (cd "$root" && cargo build --release -p aflos)
  (cd "$root/aflos-wa" && cargo build --release)
  (cd "$root/aflos-dc" && cargo build --release)
  node="${MESH_NODE:-$root/../Mesh-Node}"
  engine="$node/target/release/mesh-node"
  if [[ -f "$node/Cargo.toml" ]]; then
    (cd "$node" && cargo build --release -p mesh-node)
  fi
  app="$root/target/release/aflos" wa="$root/aflos-wa/target/release/aflos-wa"
  dc="$root/aflos-dc/target/release/aflos-dc" hicolor="$root/crates/aflos/assets/hicolor"
fi

mkdir -p "$bin" "$apps"
install -m755 "$app" "$bin/aflos"
# Over a Mesh install (its menu entry is the sign): drop its files, and point
# its ~/.local/bin/mesh at aflos, which Mesh 0.3.1's updater starts after
# installing. Without a Mesh install, a mesh that's there is left alone.
if [[ -e "$apps/mesh.desktop" ]] || ours_mesh_link; then
  rm -f "$bin/mesh-wa" "$bin/mesh-dc" "$apps/mesh.desktop" "$icons/256x256/apps/mesh.png"
  ln -sfn aflos "$bin/mesh"
fi
install -m755 "$wa" "$bin/aflos-wa"
if grep -qs '^discord_native_wrapper' "$configs/aflos/config.toml" "$configs/mesh/config.toml"; then
  install -m755 "$dc" "$bin/aflos-dc"
else
  echo "aflos-dc not installed: set discord_native_wrapper in $configs/aflos/config.toml first" >&2
fi
if [[ -x "$engine" ]]; then
  install -m755 "$engine" "$bin/mesh-node"
else
  echo "mesh-node not installed: no Mesh-Node checkout (set MESH_NODE)" >&2
fi
for s in $sizes; do
  install -Dm644 "$hicolor/${s}x${s}/apps/aflos.png" "$icons/${s}x${s}/apps/aflos.png"
done
install -Dm644 "$hicolor/scalable/apps/aflos.svg" "$icons/scalable/apps/aflos.svg"
cat > "$apps/aflos.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Aflos
GenericName=Chat client
Comment=Encrypted. Decentralized. Yours.
Exec="$bin/aflos"
Icon=aflos
StartupWMClass=aflos
Terminal=false
Categories=Network;InstantMessaging;Chat;
EOF
update-desktop-database "$apps" 2>/dev/null || true
# A newer folder date makes the desktop rescan the icons. (No icon-theme.cache
# here: a cache in your home folder goes stale and hides other apps' icons.)
touch "$icons"
echo "installed to $bin"
echo "Aflos is for people 18 and over (see TERMS.md)."
