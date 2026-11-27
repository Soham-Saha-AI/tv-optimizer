# Portable Windows update — 27 November 2026

TV Optimizer **0.1.2** focuses on plain-English ease of use, DNS control, and JioHotstar account safety while continuing to bundle **SwiftOS v3 build 25** (`3.0.2-swiftos`, `versionCode 25`), build 23 (`3.0-swiftos`), and build 22 (`liquid-glass.apk`).

## Highlights
- **Default TV DNS (Turn off Private DNS):** Clears `global/private_dns_mode` and `global/private_dns_specifier` overrides back to stock Android TV Automatic default DNS, verifies validated connectivity, rolls back automatically if connectivity fails, and keeps a Restore Centre undo point. Includes a quick **Turn Private DNS off (Default TV)** button right in Ads & privacy whenever a Private DNS setting is active.
- **JioHotstar Safe Guard & Custom Private DNS:** Detects `in.startv.hotstar` / `com.jio.media.ondemand` on connected TVs, defaults JioHotstar Safe Guard **ON** to help prevent accidental 24-hour `NM-1001_429` / `PB_ANDROID_NM-4290` account locks from public AdGuard/HaGeZi filters, lets users toggle Guard On/Off anytime, supports custom DoT hostnames (NextDNS / Control D / personal AdGuard DNS), and provides a 1-click **Copy allowlist domains** helper.
- **Easier TV connection & Setup:** Plain-English error explanations for `(10060)` timeouts (checking 2.4 GHz vs 5 GHz Wi-Fi splits and TV Network IP) and `(10061)` refusals (re-toggling USB/Network debugging or using Wireless debugging pairing), cleans up ghost `offline` ADB entries on failed connects, widens LAN discovery probe timeout (`220ms` to `400ms`), promotes **Scan this TV** to a primary button, and remembers valid connected/scanned TV IPs locally.

## Verification
- **TV Optimizer 0.1.2:** Passed 68 native Rust unit tests (`cargo test`), 23 frontend Vitest tests, TypeScript/Vite production build, `Verify-Package.ps1` (`743` packaged files), manual check on Windows 11, and GitHub Actions `windows-latest` CI.
- **SwiftOS v3 build 25 (`3.0.2-swiftos`):** Passed 260 Android JVM unit tests, release lint, R8, signer continuity, and 156 lossless-PNG pixel checks; verified running with `v0.1.1` on `MiTV-AXSO1`.
