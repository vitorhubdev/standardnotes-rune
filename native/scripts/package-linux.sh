#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml)
test -n "$version"
binary="${CARGO_TARGET_DIR:-target}/release/standardnotes-rune"
test -x "$binary"
name="standardnotes-rune-${version}-linux-x86_64"
mkdir -p "dist/$name" "dist/deb-root/DEBIAN" "dist/deb-root/usr/bin" \
  "dist/deb-root/usr/share/applications" "dist/deb-root/usr/share/icons/hicolor/scalable/apps" \
  "dist/deb-root/usr/share/doc/standardnotes-rune"
install -m 755 "$binary" "dist/$name/standardnotes-rune"
install -m 644 ../LICENSE README.md MIGRATION-COVERAGE.md "dist/$name/"
cp -R third-party "dist/$name/third-party"
rust_sysroot=$(rustc --print sysroot)
mkdir -p "dist/$name/third-party/rust-stdlib"
cp "$rust_sysroot/share/doc/rust/COPYRIGHT-library.html" "dist/$name/third-party/rust-stdlib/"
cp -R "$rust_sysroot/share/doc/rust/licenses" "dist/$name/third-party/rust-stdlib/licenses"
rustc -Vv > "dist/$name/BUILD-INFO.txt"
git rev-parse HEAD >> "dist/$name/BUILD-INFO.txt"
tar -czf "dist/$name.tar.gz" -C dist "$name"
install -m 755 "$binary" dist/deb-root/usr/bin/standardnotes-rune
install -m 644 packaging/io.github.vitorhubdev.StandardNotesRune.desktop dist/deb-root/usr/share/applications/
install -m 644 packaging/standardnotes-rune.svg dist/deb-root/usr/share/icons/hicolor/scalable/apps/
install -m 644 ../LICENSE dist/deb-root/usr/share/doc/standardnotes-rune/copyright
install -m 644 README.md MIGRATION-COVERAGE.md dist/deb-root/usr/share/doc/standardnotes-rune/
cp -R "dist/$name/third-party" dist/deb-root/usr/share/doc/standardnotes-rune/third-party
cp "dist/$name/BUILD-INFO.txt" dist/deb-root/usr/share/doc/standardnotes-rune/
deb_version="${version/-/~}"
cat > dist/deb-root/DEBIAN/control <<EOF
Package: standardnotes-rune
Version: $deb_version
Section: editors
Priority: optional
Architecture: amd64
Maintainer: vitorhubdev <noreply@github.com>
Homepage: https://github.com/vitorhubdev/standardnotes-rune
Depends: libc6 (>= 2.39), libgcc-s1, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libwayland-cursor0, libwayland-egl1, libvulkan1, libx11-6, libxcb1, libxcursor1, libx11-xcb1, libxi6
Recommends: mesa-vulkan-drivers | vulkan-icd, xdg-desktop-portal, xdg-desktop-portal-gtk
Description: Native Rust/Rune notes client (experimental migration)
 An independent native desktop client with encrypted local storage.
 Consult MIGRATION-COVERAGE.md before using existing Standard Notes data.
EOF
dpkg-deb --root-owner-group --build dist/deb-root "dist/$name.deb"
(
  cd dist
  sha256sum "$name.tar.gz" "$name.deb" > "SHA256SUMS-linux.txt"
)
# Keep only distributables in dist, so CI does not upload temporary package trees.
rm -r -- "dist/$name" dist/deb-root
