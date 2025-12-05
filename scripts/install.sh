#!/bin/sh
# curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/honhimW/ratisui/main/scripts/install.sh | sh

set -e

AUTO_YES=false
for arg in "$@"; do
  if [ "$arg" = "-y" ]; then
    AUTO_YES=true
    break
  fi
done

REPO="honhimW/ratisui"
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS-$ARCH" in
  Linux-x86_64)   ASSET="ratisui-linux-amd64.tar.gz" ;;
  Linux-aarch64)  ASSET="ratisui-linux-aarch64.tar.gz" ;;
  Darwin-x86_64)  ASSET="ratisui-macos-intel.tar.gz" ;;
  Darwin-arm64)   ASSET="ratisui-macos-aarch64.tar.gz" ;;
  *) echo "Unsupported platform: $OS-$ARCH"; exit 1 ;;
esac

API_URL="https://api.github.com/repos/$REPO/releases/latest"

RELEASE_METADATA=$(curl --proto '=https' --tlsv1.2 -sSfL $API_URL)

get_digest() {
  echo "$RELEASE_METADATA" | awk -v name="$1" '
    $0 ~ "\"name\": \"" name "\"" {found=1}
    found && /"digest":/ {
      match($0, /sha256:([a-f0-9]{64})/, arr);
      print arr[1];
      exit
    }'
}
get_size_human() {
  echo "$RELEASE_METADATA" | awk -v name="$1" '
    $0 ~ "\"name\": \"" name "\"" {found=1}
    found && /"size":/ {
      match($0, /"size": *([0-9]+)[,}]/, arr);
      size = arr[1];
      if (size < 1024) {
        printf "%.0f B\n", size;
      } else if (size < 1024 * 1024) {
        printf "%.2f KB\n", size / 1024;
      } else {
        printf "%.2f MB\n", size / (1024 * 1024);
      }
      exit
    }'
}
get_created_at() {
  echo "$RELEASE_METADATA" | awk -v name="$1" '
    $0 ~ "\"name\": \"" name "\"" {found=1}
    found && /"created_at":/ {
      match($0, /"created_at": *"([^"]+)"/, arr);
      print arr[1];
      exit
    }'
}

TAG_NAME=$(echo "$RELEASE_METADATA" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
DOWNLOAD_URL="https://github.com/$REPO/releases/download/$TAG_NAME/$ASSET"
SHA_256=$(get_digest $ASSET)
SIZE=$(get_size_human $ASSET)
CREATED_AT=$(get_created_at $ASSET)

echo "OS        : $OS-$ARCH"
echo "Resource  : $DOWNLOAD_URL"
echo "Sha256    : $SHA_256"
echo "Size      : $SIZE"
echo "Created at: $CREATED_AT"

# Confirmation block
if [ "$AUTO_YES" = false ]; then
  read -p "Continue? (y/n): " choice < /dev/tty
  case "$choice" in
    y|Y ) echo "Proceeding..." ;;
    n|N ) echo "Aborted."; exit 1 ;;
    * ) echo "Invalid input. Aborted."; exit 1 ;;
  esac
else
  echo "Auto-confirm enabled with '-y'."
fi

# Do download
TMP_DIR="$(mktemp -d)"

# Do finally
cleanup() {
  rm -rf "$TMP_DIR"
  echo "Cleanup temporary directory succeed."
}
trap cleanup EXIT

echo "Downloading ..."
curl --proto '=https' --tlsv1.2 -#SfL "$DOWNLOAD_URL" -o "$TMP_DIR/$ASSET"

if [ "$OS" = "Linux" ]; then
  ASSET_SHA256=$(sha256sum "$TMP_DIR/$ASSET" | awk '{print $1}')
else
  ASSET_SHA256=$(shasum -a 256 "$TMP_DIR/$ASSET" | awk '{print $1}')
fi

if [ "$ASSET_SHA256" = "$SHA_256" ]; then
  echo "Checksum OK!"
else
  echo "Checksum mismatch!" >&2
  exit 1
fi

tar -xzf "$TMP_DIR/$ASSET" -C "$TMP_DIR"

# Install in ~/.local/bin
INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"
mv "$TMP_DIR/ratisui" "$INSTALL_DIR/ratisui"
chmod +x "$INSTALL_DIR/ratisui"

# Hint
if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
  echo "make sure '$INSTALL_DIR' is added to PATH."
  echo ""
  echo '  export PATH="$HOME/.local/bin:$PATH"'
  echo ""
fi

# Validation
"$INSTALL_DIR/ratisui" --version

echo "Complete!"
