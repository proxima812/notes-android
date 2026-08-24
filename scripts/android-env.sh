#!/usr/bin/env bash
# Source this file before any Android command:
#   source ./scripts/android-env.sh
#
# Pinned versions live here so that a broken toolchain fails loudly and early
# instead of producing a confusing Gradle or NDK error deep into a build.
#
# Anything belonging to this machine rather than to the project — the signing
# passwords, a JDK somewhere else, an SDK on another disk — comes from `.env`,
# which is git-ignored. See `.env.example`.

_android_env_root="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)"
_android_env_file="$_android_env_root/.env"

if [ -f "$_android_env_file" ]; then
  # `set -a` exports every assignment the file makes; nothing in it is executed,
  # because the file is read as assignments and comments only.
  set -a
  # shellcheck disable=SC1090
  . "$_android_env_file"
  set +a
fi

NDK_VERSION="${ANDROID_NDK_VERSION:-27.3.13750724}"
JDK_PREFIX="${JAVA_HOME:-/opt/homebrew/opt/openjdk@21}"

export PATH="$HOME/.cargo/bin:$PATH"
export JAVA_HOME="$JDK_PREFIX"
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
export NDK_HOME="$ANDROID_HOME/ndk/$NDK_VERSION"
export PATH="$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$PATH"

_android_env_problem=0

# Gradle reads the signing material from a properties file, so the four lines in
# `.env` are written out into one. Rewritten every time rather than checked:
# a password changed in `.env` and a stale file beside it is exactly the failure
# this indirection exists to prevent.
if [ -n "${ANDROID_KEYSTORE_FILE:-}" ]; then
  _android_env_keystore="$_android_env_root/src-tauri/gen/android/keystore.properties"
  if [ -d "$(dirname "$_android_env_keystore")" ]; then
    umask 077
    {
      echo "# Собран из .env скриптом scripts/android-env.sh. Править .env."
      echo "storeFile=$ANDROID_KEYSTORE_FILE"
      echo "storePassword=$ANDROID_KEYSTORE_PASSWORD"
      echo "keyAlias=$ANDROID_KEY_ALIAS"
      echo "keyPassword=$ANDROID_KEY_PASSWORD"
    } >"$_android_env_keystore"
    chmod 600 "$_android_env_keystore"
  fi
  unset _android_env_keystore

  if [ ! -f "$ANDROID_KEYSTORE_FILE" ]; then
    echo "android-env: keystore не найден: $ANDROID_KEYSTORE_FILE" >&2
    echo "  проверьте ANDROID_KEYSTORE_FILE в .env" >&2
    _android_env_problem=1
  fi
fi

if [ ! -x "$JAVA_HOME/bin/java" ]; then
  echo "android-env: JDK 21 не найден в $JAVA_HOME" >&2
  echo "  установите: brew install openjdk@21" >&2
  _android_env_problem=1
fi

if [ ! -d "$NDK_HOME" ]; then
  echo "android-env: NDK $NDK_VERSION не найден в $NDK_HOME" >&2
  echo "  установите: sdkmanager --sdk_root=\"\$ANDROID_HOME\" \"ndk;$NDK_VERSION\"" >&2
  _android_env_problem=1
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "android-env: cargo не найден в PATH" >&2
  _android_env_problem=1
elif ! rustup target list --installed 2>/dev/null | grep -q '^aarch64-linux-android$'; then
  echo "android-env: отсутствует Rust target aarch64-linux-android" >&2
  echo "  установите: rustup target add aarch64-linux-android" >&2
  _android_env_problem=1
fi

if [ "$_android_env_problem" -eq 0 ]; then
  echo "android-env: ok — JDK $("$JAVA_HOME/bin/java" -version 2>&1 | head -1 | sed 's/.*"\(.*\)".*/\1/'), NDK $NDK_VERSION"
fi

unset _android_env_problem _android_env_root _android_env_file
