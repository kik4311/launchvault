#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/version *= *"\(.*\)"/\1/')
echo "==> Сборка LaunchVault v$VERSION на $(uname -m) =="

mkdir -p target/release target/debian target/generate-rpm

echo "==> Linux бинарник (release) =="
cargo build --release

echo "==> deb пакет =="
cargo deb --no-build --output target/debian

echo "==> rpm пакет =="
cargo generate-rpm

echo "==> AppImage =="
rm -rf AppDir
mkdir -p AppDir/usr/bin
cp target/release/launchvault AppDir/usr/bin/
cp src/launchvault.desktop AppDir/
cp src/launchvault.svg AppDir/
magick AppDir/launchvault.svg AppDir/launchvault.png
cat > AppDir/AppRun << 'EOF'
#!/bin/sh
export PATH="$APPDIR/usr/bin:$PATH"
exec "$APPDIR/usr/bin/launchvault" "$@"
EOF
chmod +x AppDir/AppRun
"$HOME/.local/bin/linuxdeploy" --appdir AppDir --output appimage
rm -rf AppDir

echo
echo "==> Готово! Артефакты:"
ls -lh LaunchVault-x86_64.AppImage \
      target/debian/*.deb \
      target/generate-rpm/*.rpm
echo
echo "Windows-установщик (Inno Setup) собирается в CI: github.com/kik4311/launchvault/actions"
