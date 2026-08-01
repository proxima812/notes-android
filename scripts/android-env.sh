#!/usr/bin/env bash
# Source this file before any Android command:
#   source ./scripts/android-env.sh
#
# Pinned versions live here so that a broken toolchain fails loudly and early
# instead of producing a confusing Gradle or NDK error deep into a build.

NDK_VERSION="27.3.13750724"
JDK_PREFIX="/opt/homebrew/opt/openjdk@21"

export PATH="$HOME/.cargo/bin:$PATH"
export JAVA_HOME="$JDK_PREFIX"
export ANDROID_HOME="$HOME/Library/Android/sdk"
export NDK_HOME="$ANDROID_HOME/ndk/$NDK_VERSION"
export PATH="$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$PATH"

_android_env_problem=0

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

unset _android_env_problem
