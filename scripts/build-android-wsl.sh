#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CLIENT_DIR="$REPO_ROOT/privchat-client"
SDK="${ANDROID_HOME:-/opt/android-sdk}"
NDK="${NDK_HOME:-$SDK/ndk/27.2.12479018}"
JAVA="${JAVA_HOME:-/usr/lib/jvm/java-21-openjdk}"

for path in "$SDK" "$NDK" "$JAVA"; do
  [ -d "$path" ] || { echo "missing Android toolchain path: $path" >&2; exit 1; }
done

export ANDROID_HOME="$SDK" ANDROID_SDK_ROOT="$SDK" NDK_HOME="$NDK" JAVA_HOME="$JAVA"
export PATH="$SDK/cmdline-tools/latest/bin:$SDK/platform-tools:$PATH"
cd "$CLIENT_DIR"
npm install --include=optional
"$CLIENT_DIR/node_modules/.bin/tauri" android build --debug --target x86_64
