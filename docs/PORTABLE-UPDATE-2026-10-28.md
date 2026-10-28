# Portable Windows update — 28 October 2026

The native compiler is working again. The earlier Application Control block no longer prevents this build. No Windows security setting was changed by this update.

The Windows release includes separate available recommendations and Optimizations found, clear review/apply confirmation, verified per-user Close app behavior, individual launcher choices, and the local Wallpaper Engine. Photo transfers check TV identity, validate bounded JPEGs, verify transfer checksums, report partial results, and avoid automatic retries. PC wallpaper controls are limited to the validated Mi TV profile and primary Android user.

Validation completed: 63 native Rust tests and the TypeScript/Vite release build. The previous frontend checks passed 23 tests and isolated browser flows at 1280 × 900 and 920 × 760. Android build24 previously passed 257 tests, lint, R8, signer continuity and bounded Mi TV wallpaper checks. No TV tests or TV mutations are performed for this portable-package update, as requested. A clean Windows VM, long-duration playback and physical remote checks remain outside this validation.

The portable package includes the fixed WebView2 runtime, ADB and libraries, offline rules, signed build24, unchanged tested build23 and build22, help and integrity checks. Original CleanYourTV remains an optional online preparation. The update preserves the existing portable folder's data directory and keeps backups of replaced application files and earlier ZIPs. Distribution ZIPs contain a fresh data directory, not the owner's TV history or browser profile.

This document supersedes the Windows compilation-block status recorded in the earlier update and build23 notes. It does not change their historical TV measurements or compatibility limits.
