# Portable Windows update — 11 November 2026

TV Optimizer **0.1.1** updates the bundled SwiftOS launcher to **3.0.2 (build 25)** while retaining the unchanged tested build 23 (`3.0-swiftos`) and SwiftOS 2.7 (`2.7-swiftos`, build 22) APKs separately.

## Changes in this release

- **SwiftOS v3 build 25 (`3.0.2-swiftos`, `versionCode 25`):**
  - Fixes cold-boot persistence so on-TV theme choices (`ThemePicker`) and icon background opacity / layout customizations survive full TV power cycles even when external storage mounts asynchronously.
  - Commits TV settings synchronously via `.commit()` and keeps fallback copies of `config.json` and `wallpaper.jpg` in private internal app storage.
  - Protects TV customizations from accidental resets when no external CLI `theme_revision` file is pushed.
  - Includes App Library `RecyclerView` fixed-size and 18-item view-cache tuning.
- **TV Optimizer 0.1.1:**
  - Pins the reviewed SwiftOS v3 build 25 SHA-256 (`c3209b1194650f05056ea40ac4b1ed66a300249e3cae81f34de1036778b50859`) and updates package version to `0.1.1`.
  - Adds a reproducible Windows GitHub Actions build and portable packaging workflow (`.github/workflows/release-windows.yml`).

## Validation scope

- **SwiftOS v3 build 25:** 260 Android JVM tests (257 core tests + 3 persistence tests), release lint, R8 optimization, APK signer continuity (`e0e52087919f09bb6bfe5d5fc7e986b643a2bf1f3d49b40e8827f87e3ac5f986`) and 156 pixel-integrity artwork checks passed. The underlying cold-boot persistence and App Library fixes were smoke-tested on one Android TV as build 24 on 2 November 2026; the renumbered build 25 APK has not yet been re-installed onto hardware.
- **TV Optimizer 0.1.1:** Built on GitHub Actions `windows-latest` running 63 native Rust tests, 23 frontend Vitest tests, TypeScript/Vite release build, and `Verify-Package.ps1` integrity verification. Clean Windows VM execution and new physical TV testing of this updated portable bundle remain outside automated CI validation.
