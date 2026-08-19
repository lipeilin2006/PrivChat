#!/usr/bin/env bash
# ============================================================
# PrivChat — Windows x86_64 桌面客户端编译脚本
#
# 用法：
#   bash scripts/build-windows-x86_64.sh [--mailbox]
#
# 默认要求编译工具链完整：
#   - Rust stable + x86_64-pc-windows-msvc 目标
#   - Node.js + npm
#   - Tauri CLI（随 privchat-client 的 devDependencies 安装）
#
# 产物复制到工作区 target/ 目录：
#   target/release/privchat-client.exe     （Tauri 构建原始输出）
#   target/windows-x86_64/privchat-client.exe   （归档副本）
# ============================================================
set -euo pipefail

# —— 定位仓库根目录（脚本位于 $REPO_ROOT/scripts/）——
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CLIENT_DIR="$REPO_ROOT/privchat-client"
ARCH_DIR="$REPO_ROOT/target/windows-x86_64"

BUILD_MAILBOX=0
for arg in "$@"; do
  case "$arg" in
    --mailbox) BUILD_MAILBOX=1 ;;
    *) echo "unknown arg: $arg" >&2; exit 1 ;;
  esac
done

echo "[build] PrivChat Windows x86_64"
echo "[build] repo: $REPO_ROOT"
mkdir -p "$ARCH_DIR"

# —— 前端依赖 ——
if [ ! -d "$CLIENT_DIR/node_modules" ]; then
  echo "[build] installing frontend deps"
  (cd "$CLIENT_DIR" && npm install)
fi

# —— Tauri 桌面客户端（no-bundle：仅产 exe，不生成安装包）——
echo "[build] building tauri client (this may take a while)"
(cd "$CLIENT_DIR" && npm run tauri build -- --no-bundle)

# —— 复制产物到 target/windows-x86_64/ ——
EXE="$REPO_ROOT/target/release/privchat-client.exe"
if [ ! -f "$EXE" ]; then
  echo "[build] ERROR: expected $EXE but not found" >&2
  exit 1
fi
cp -f "$EXE" "$ARCH_DIR/privchat-client.exe"
echo "[build] -> $ARCH_DIR/privchat-client.exe"

# —— mailbox 节点（可选，--mailbox）——
if [ "$BUILD_MAILBOX" -eq 1 ]; then
  echo "[build] building mailbox node"
  (cd "$REPO_ROOT" && cargo build --release -p privchat-mailbox)
  MB="$REPO_ROOT/target/release/privchat-mailbox.exe"
  if [ ! -f "$MB" ]; then
    echo "[build] ERROR: expected $MB but not found" >&2
    exit 1
  fi
  cp -f "$MB" "$ARCH_DIR/privchat-mailbox.exe"
  echo "[build] -> $ARCH_DIR/privchat-mailbox.exe"
fi

echo "[build] done."
ls -la "$ARCH_DIR"