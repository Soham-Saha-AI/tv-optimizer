# Portable build and distribution

The build toolchain lives in `.tools` inside this workspace. It makes no machine-wide compiler installation and does not modify the Windows PATH permanently. The compiler and source caches are never copied into a release.

## Prepared components

- Rust 1.99.1 for `x86_64-pc-windows-msvc`, with local `RUSTUP_HOME=.tools/rustup` and `CARGO_HOME=.tools/cargo`.
- Microsoft MSVC 14.44.35207 and Windows SDK 10.0.26100.0 extracted from Microsoft's signed Visual Studio packages. The bootstrap helper validates each archive against the SHA256 provided by Microsoft's package manifest.
- Microsoft WebView2 Fixed Version Runtime 156.0.4314.42 x64, downloaded directly from the official Microsoft download metadata. Its source URL and cabinet SHA256 are recorded in `.tools/webview2-source.json`. `msedgewebview2.exe` has a valid Microsoft Authenticode signature.
- Android Platform-Tools 37.0.1 from the existing workspace, including ADB's required DLLs and complete NOTICE.
- Existing workspace JDK, Android SDK34 and Gradle8.9 for launcher builds. Signing keys remain outside the distribution.

Bootstrap commands from the workspace parent directory:

```powershell
& .\tv-optimizer\scripts\Prepare-Toolchain.ps1
& .\tv-optimizer\scripts\Prepare-Toolchain.ps1 -NativeOnly
```

The MSVC downloader is the reviewed `portable-msvc.py` helper by Mārtiņš Možeiko, downloaded from https://gist.github.com/mmozeiko/7f3162ec2988e81e56d5c4e22cde9977. It extracts compiler packages and performs administrative MSI extraction into `.tools/msvc`; it does not register Visual Studio or install system services. The downloaded compiler is subject to Microsoft's Visual Studio Build Tools license.

## Build and test

```powershell
& .\tv-optimizer\scripts\Build-App.ps1 -CheckOnly
& .\tv-optimizer\scripts\Build-App.ps1 -Test
& .\tv-optimizer\scripts\Build-App.ps1 -Release
```

The script sets the native compiler, SDK include/library paths and static CRT flags only for its process. Release compilation builds the frontend first and embeds the resulting frontend assets in the executable. `Cargo.lock` and the frontend lockfile pin application dependencies. Internet access is needed only to prepare development dependencies and compile previously uncached versions.

## Launcher artifacts

`Build-OriginalLauncher.ps1` downloads upstream CleanYourTV v2.7.0 and supplies `-PlauncherApplicationId=com.gathin.tvhome.original`. It changes no upstream source files. This preserves the upstream Home interface and allows coexistence with SwiftOS, which retains the custom launcher's existing package identity. The original launcher requires Android 8/API26 or newer; its versionCode is8 and targetSDK is34. Its metadata and source-archive SHA256 are recorded in `launchers/original-manifest.json`.

`Build-CustomLauncher.ps1` owns the active SwiftOS code22 compatibility build and keeps the prior Liquid Glass code21 APK as a recovery artifact. The legacy APK must never be silently substituted for an installed code22 build.

## Package

```powershell
& .\tv-optimizer\scripts\Package-Portable.ps1 -NoZip
& .\tv-optimizer\scripts\Package-Portable.ps1
```

Every invocation stages a fresh folder under `.tools/package-staging` so a TV's real history is never deleted or accidentally redistributed. The final ZIP is `releases/TV-Optimizer-0.1.0-Windows-x64.zip` and contains the `TVOptimizer` folder. A pre-existing ZIP is preserved; move it before generating another release with the same version.

The folder contains the standalone executable, complete fixed WebView2 runtime, ADB and DLLs, SwiftOS and its recovery APK, offline profiles, beginner help, a writable empty data folder, applicable notices, dependency license texts and a SHA256 integrity manifest. Original CleanYourTV is prepared only when the user requests it online. The shipped package contains no compiler, private signing key, device trust key, personal export or source cache.

The standalone verifier can be run with Windows PowerShell or PowerShell7:

```powershell
& '.\TVOptimizer\Verify-Package.ps1'
```

Checksums detect missing or changed packaged files. They do not constitute code signing. Authenticode signing of the final application requires an owner-supplied signing certificate and is a separate release step. The app itself does not install services, drivers, runtimes or tasks.
