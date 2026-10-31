#!/usr/bin/env bash
# macOS 전용. Rust 코어를 iOS 용으로 빌드해 NoteFFI.xcframework 와 Swift 바인딩을 만든다.
set -euo pipefail
step() { echo "::group::$*"; echo "[build-ios] $(date +%T) $*"; }
cd "$(dirname "$0")/.."
command -v xcodebuild >/dev/null || { echo "xcodebuild 필요 (macOS + Xcode)"; exit 1; }
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios

# QuickJS(C) 컴파일과 Rust 링크의 최소 iOS 버전을 앱(project.yml)과 같게 맞춘다.
# 안 맞추면 링크 시 ___chkstk_darwin 미정의 오류가 난다.
export IPHONEOS_DEPLOYMENT_TARGET=17.0

# iOS 는 정적 라이브러리만 필요하다(cdylib 링크는 건너뜀)
for t in aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios; do
  step "rust staticlib $t"
  cargo rustc -p note-ffi --lib --release --target "$t" --crate-type staticlib
done

step lipo
# 시뮬레이터용 fat 라이브러리
mkdir -p target/ios-sim
lipo -create target/aarch64-apple-ios-sim/release/libnote_ffi.a target/x86_64-apple-ios/release/libnote_ffi.a -output target/ios-sim/libnote_ffi.a

step "bindings(host build)"
# 바인딩 생성(호스트 빌드 라이브러리 기준). 호스트 빌드에는 iOS 배포 타깃을 적용하지 않는다.
unset IPHONEOS_DEPLOYMENT_TARGET
cargo build -p note-ffi
lib=target/debug/libnote_ffi.dylib
rm -rf target/bindings && mkdir -p target/bindings/swift
cargo run -q -p note-ffi --bin uniffi-bindgen -- generate --library "$lib" --language swift --out-dir target/bindings/swift --no-format

hdr=target/bindings/headers
rm -rf "$hdr" && mkdir -p "$hdr"
cp target/bindings/swift/note_ffiFFI.h "$hdr/"
cp target/bindings/swift/note_ffiFFI.modulemap "$hdr/module.modulemap"

step xcframework
rm -rf ios/NoteFFI.xcframework
xcodebuild -create-xcframework \
  -library target/aarch64-apple-ios/release/libnote_ffi.a -headers "$hdr" \
  -library target/ios-sim/libnote_ffi.a -headers "$hdr" \
  -output ios/NoteFFI.xcframework

mkdir -p ios/PluginNote/Generated
cp target/bindings/swift/note_ffi.swift ios/PluginNote/Generated/note_ffi.swift
echo "완료: ios/NoteFFI.xcframework, ios/PluginNote/Generated/note_ffi.swift"
