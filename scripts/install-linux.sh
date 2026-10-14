#!/usr/bin/env bash
set -Eeuo pipefail

readonly REPOSITORY="vandunxg/sql-judge"
readonly API_URL="https://api.github.com/repos/${REPOSITORY}/releases/latest"
readonly CHECKSUM_ASSET="sql-judge-linux-x86_64.SHA256SUMS"

mode=system
case "${1:-}" in
  --user) mode=user ;;
  --system|"") mode=system ;;
  --help|-h)
    cat <<'HELP'
Install SQL Judge from the latest verified GitHub release.

Usage: install-linux.sh [--system|--user]

  --system  Install for all users (default; uses sudo)
  --user    Install for the current user (does not use sudo; AppImage)
HELP
    exit 0
    ;;
  *)
    echo "Unknown option: $1 (use --help)" >&2
    exit 2
    ;;
esac

if [ "$(uname -s)" != Linux ]; then
  echo "This installer supports Linux only." >&2
  exit 1
fi

arch=$(uname -m)
case "$arch" in
  x86_64|amd64) asset_arch=x86_64 ;;
  *)
    echo "Unsupported Linux architecture: $arch (only x86_64 is currently published)." >&2
    exit 1
    ;;
esac

command -v curl >/dev/null 2>&1 || {
  echo "curl is required. Install it with your distribution package manager and retry." >&2
  exit 1
}

if [ "$mode" = system ] && ! command -v sudo >/dev/null 2>&1; then
  echo "sudo is required for system installation; use --user for a user-local install." >&2
  exit 1
fi

tmp_dir=$(mktemp -d)
cleanup() { rm -rf "$tmp_dir"; }
trap cleanup EXIT

download() {
  curl -fsSL --retry 3 "$1" -o "$2"
}

asset_url() {
  local name_pattern=$1
  sed 's/},{/}\n{/g' "$tmp_dir/release.json" |
    grep -E '"name"[[:space:]]*:[[:space:]]*"[^"]*'"$name_pattern"'[^\"]*"' |
    sed -n 's/.*"browser_download_url"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' |
    head -n 1 || true
}

download "$API_URL" "$tmp_dir/release.json" || {
  echo "Could not read the latest SQL Judge release from GitHub." >&2
  exit 1
}

checksum_url=$(asset_url "$CHECKSUM_ASSET")
if [ -z "$checksum_url" ]; then
  echo "The release does not publish the required checksum manifest; refusing to install." >&2
  exit 1
fi
download "$checksum_url" "$tmp_dir/SHA256SUMS"

is_debian_family=false
if [ -r /etc/os-release ]; then
  # shellcheck disable=SC1091
  . /etc/os-release
  case "${ID:-} ${ID_LIKE:-}" in
    *debian*|*ubuntu*) is_debian_family=true ;;
  esac
fi

install_kind=appimage
if [ "$mode" = system ] && [ "$is_debian_family" = true ] && command -v dpkg >/dev/null 2>&1; then
  install_kind=deb
fi

if [ "$install_kind" = deb ]; then
  asset_url=$(asset_url "linux-${asset_arch}.*\\.(deb)")
else
  asset_url=$(asset_url "linux-${asset_arch}.*(AppImage)")
fi
if [ -z "$asset_url" ]; then
  echo "No matching verified $install_kind asset was found for Linux $asset_arch." >&2
  exit 1
fi

asset_name=${asset_url##*/}
asset_path="$tmp_dir/$asset_name"
download "$asset_url" "$asset_path"

checksum_line=$(grep -E "[[:space:]]${asset_name}$" "$tmp_dir/SHA256SUMS" | head -n 1 || true)
if [ -z "$checksum_line" ]; then
  echo "No checksum was published for $asset_name; refusing to install." >&2
  exit 1
fi
printf '%s\n' "$checksum_line" > "$tmp_dir/one.SHA256SUM"
if ! (cd "$tmp_dir" && sha256sum -c one.SHA256SUM >/dev/null); then
  echo "Checksum verification failed for $asset_name; refusing to install." >&2
  exit 1
fi

if [ "$install_kind" = deb ]; then
  sudo dpkg -i "$asset_path" || sudo apt-get install -f -y
  echo "SQL Judge installed from verified DEB package."
  exit 0
fi

if [ "$mode" = user ]; then
  bin_dir="${XDG_BIN_HOME:-$HOME/.local/bin}"
  app_dir="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
  mkdir -p "$bin_dir" "$app_dir"
else
  bin_dir=/usr/local/bin
  app_dir=/usr/share/applications
  sudo mkdir -p /opt/sql-judge "$app_dir"
  sudo install -m 0755 "$asset_path" /opt/sql-judge/sql-judge.AppImage
fi

if [ "$mode" = user ]; then
  install -m 0755 "$asset_path" "$bin_dir/sql-judge"
else
  sudo ln -sf /opt/sql-judge/sql-judge.AppImage "$bin_dir/sql-judge"
fi

desktop_entry=$(cat <<EOF
[Desktop Entry]
Name=SQL Judge
Comment=Offline-first SQL learning environment
Exec=$bin_dir/sql-judge
Terminal=false
Type=Application
Categories=Development;Education;
EOF
)
if [ "$mode" = user ]; then
  printf '%s\n' "$desktop_entry" > "$app_dir/sql-judge.desktop"
  chmod 0644 "$app_dir/sql-judge.desktop"
else
  printf '%s\n' "$desktop_entry" | sudo tee "$app_dir/sql-judge.desktop" >/dev/null
  sudo chmod 0644 "$app_dir/sql-judge.desktop"
fi

echo "SQL Judge installed from verified AppImage."
