#!/bin/sh
# Builds manual.pdf and the README's gallery images. Only fonts bundled with Typst are used, so
# the output is the same on every machine.
set -e
cd "$(dirname "$0")"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' typst.toml)

# The gallery imports @preview/pele as users do; serve it from this checkout.
packages=$(mktemp -d)
trap 'rm -rf "$packages"' EXIT
mkdir -p "$packages/preview/pele"
ln -s "$PWD" "$packages/preview/pele/$version"

typst compile --root . --ignore-system-fonts manual.typ manual.pdf
for file in gallery/*.typ; do
  typst compile --package-path "$packages" --ignore-system-fonts --ppi 200 "$file" "${file%.typ}.png"
done
