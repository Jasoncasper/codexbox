#!/bin/bash
set -e

echo "=== CodexRouter DMG Build Script ==="

# 检查 Rust
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust not found. Install: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# 检查 Node
if ! command -v npm &> /dev/null; then
    echo "❌ Node.js not found. Install: brew install node"
    exit 1
fi

export PATH="$HOME/.cargo/bin:$PATH"
ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
APP_DIR="$ROOT_DIR/apps/codex-plus-manager"

echo "📦 Installing frontend dependencies..."
cd "$APP_DIR"
npm install

echo "🔨 Building frontend..."
npx vite build

echo "🦀 Building Tauri release..."
cd "$ROOT_DIR"
cargo build --release -p codex-router-manager

echo "📱 Creating .app bundle..."
# Tauri should have created it, but let's verify
APP_BUNDLE="$ROOT_DIR/target/release/bundle/macos/CodexRouter.app"
if [ ! -d "$APP_BUNDLE" ]; then
    echo "⚠️  .app bundle not found, running tauri build..."
    cd "$APP_DIR"
    npx tauri build
fi

echo "💿 Creating DMG..."
DMG_DIR="$ROOT_DIR/target/release/bundle/dmg"
mkdir -p "$DMG_DIR"

# 创建临时目录
STAGING=$(mktemp -d)
cp -R "$APP_BUNDLE" "$STAGING/"
ln -sf /Applications "$STAGING/Applications"

# 生成 DMG
DMG_PATH="$DMG_DIR/CodexRouter_2.0.0_aarch64.dmg"
hdiutil create -volname "CodexRouter" \
    -srcfolder "$STAGING" \
    -ov -format UDZO \
    "$DMG_PATH"

# 清理
rm -rf "$STAGING"

echo ""
echo "✅ Build complete!"
echo "   DMG: $DMG_PATH"
echo "   Size: $(du -h "$DMG_PATH" | cut -f1)"
echo ""
echo "安装方式：双击 DMG，拖拽 CodexRouter 到 Applications"
