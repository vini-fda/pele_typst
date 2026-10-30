#!/bin/sh
# Copies the package's files to DIR/pele/VERSION.
#
#   ./package.sh             install as @local/pele, in Typst's local package directory
#   ./package.sh DIR         e.g. a checkout of typst/packages: ./package.sh packages/preview
set -e
cd "$(dirname "$0")"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' typst.toml)

if [ -n "$1" ]; then
  root=$1
else
  case "$(uname -s)" in
    Darwin) data="$HOME/Library/Application Support" ;;
    MINGW* | MSYS* | CYGWIN*) data="$APPDATA" ;;
    *) data="${XDG_DATA_HOME:-$HOME/.local/share}" ;;
  esac
  root="$data/typst/packages/local"
fi

target="$root/pele/$version"
rm -rf "$target"
mkdir -p "$target/gallery"
# The README, manual and gallery are shown on Typst Universe; typst.toml excludes them from the
# bundle that the compiler downloads.
cp typst.toml lib.typ pele.wasm README.md LICENSE COMPATIBILITY.md manual.pdf "$target"
cp gallery/*.typ gallery/*.png "$target/gallery"
echo "Copied pele $version to $target"
