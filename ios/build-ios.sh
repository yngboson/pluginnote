#!/usr/bin/env bash
# macOS 전용. Rust 코어를 iOS 용으로 빌드해 NoteFFI.xcframework 와 Swift 바인딩을 만든다.
set -euo pipefail
cd "$(dirname "$0")/.."
command -v xcodebuild >/dev/null || { echo "xcodebuild 필요 (macOS + Xcode)"; exit 1; }
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios

for t in aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios; do
  cargo build -p note-ffi --release --target "$t"
done

# 시뮬레이터용 fat 라이브러리
mkdir -p target/ios-sim
lipo -create target/aarch64-apple-ios-sim/release/libnote_ffi.a target/x86_64-apple-ios/release/libnote_ffi.a -output target/ios-sim/libnote_ffi.a

# 바인딩 생성(호스트 빌드 라이브러리 기준)
cargo build -p note-ffi
lib=target/debug/libnote_ffi.dylib
rm -rf target/bindings && mkdir -p target/bindings/swift
cargo run -q -p note-ffi --bin uniffi-bindgen -- generate --library "$lib" --language swift --out-dir target/bindings/swift --no-format

hdr=target/bindings/headers
rm -rf "$hdr" && mkdir -p "$hdr"
cp target/bindings/swift/note_ffiFFI.h "$hdr/"
cp target/bindings/swift/note_ffiFFI.modulemap "$hdr/module.modulemap"

rm -rf ios/NoteFFI.xcframework
xcodebuild -create-xcframework \
  -library target/aarch64-apple-ios/release/libnote_ffi.a -headers "$hdr" \
  -library target/ios-sim/libnote_ffi.a -headers "$hdr" \
  -output ios/NoteFFI.xcframework

cp target/bindings/swift/note_ffi.swift ios/PluginNote/Generated/note_ffi.swift
echo "완료: ios/NoteFFI.xcframework, ios/PluginNote/Generated/note_ffi.swift"
