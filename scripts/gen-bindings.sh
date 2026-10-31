#!/usr/bin/env bash
# Swift/Kotlin 바인딩 생성. 사용: scripts/gen-bindings.sh [출력 디렉터리]
# iOS 용 정적 라이브러리는 macOS 에서 `cargo build -p note-ffi --release --target aarch64-apple-ios` 후 xcframework 로 묶는다.
set -euo pipefail
out="${1:-target/bindings}"
cargo build -p note-ffi
lib=target/debug/libnote_ffi.so
[ -f "$lib" ] || lib=target/debug/libnote_ffi.dylib
mkdir -p "$out/swift" "$out/kotlin"
for lang in swift kotlin; do
  cargo run -q -p note-ffi --bin uniffi-bindgen -- generate --library "$lib" --language "$lang" --out-dir "$out/$lang" --no-format
done
echo "생성됨: $out"
