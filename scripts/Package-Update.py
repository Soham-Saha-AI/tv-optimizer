"""Package the tested TV APKs and a separate source update. Never packages an old EXE."""
from pathlib import Path
import hashlib
import json
import zipfile

ROOT = Path(__file__).resolve().parents[1]
RELEASES = ROOT / "releases"
APKS = {
    "SwiftOS-3.0.1-build24.apk": ("launchers/candidates/SwiftOS-3.0.1-24.apk", "0c86c2542294a5468204acf3bf79c468051d270b6997dde7dabdaf92454a572b"),
    "SwiftOS-3.0-build23-tested.apk": ("launchers/swiftos-v3-tested-23.apk", "aee129904809bef86f258d8873bbba5ee7297986b61c2e0572cb9786ec7c4aec"),
    "SwiftOS-2.7-build22.apk": ("launchers/liquid-glass.apk", "ff6f0c1f93c086e803532f40b7b2165e1c820cd62e588da233fcdf96a4123c06"),
}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def add_tree(entries, directory, target=None):
    source = ROOT / directory
    if not source.is_dir():
        raise RuntimeError(f"Missing source directory: {directory}")
    for file in sorted(source.rglob("*")):
        if file.is_file():
            name = f"{target or directory}/{file.relative_to(source).as_posix()}"
            entries[name] = file.read_bytes()


def write_zip(label, entries):
    # Inputs come exclusively from the explicit allowlist below. No histories,
    # browser profiles, private keys, TV screenshots or development executables.
    for name in entries:
        if Path(name).suffix.lower() in {".jks", ".keystore", ".p12", ".pfx", ".pem", ".exe", ".db", ".sqlite"}:
            raise RuntimeError(f"Forbidden release entry: {name}")
    entries = dict(entries)
    entries["SHA256SUMS.txt"] = "".join(f"{sha(data)}  {name}\n" for name, data in sorted(entries.items())).encode()
    output = RELEASES / f"{label}.zip"
    with zipfile.ZipFile(output, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for name, data in sorted(entries.items()):
            item = zipfile.ZipInfo(name, (2026, 10, 27, 0, 0, 0))
            item.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(item, data)
    with zipfile.ZipFile(output) as archive:
        assert archive.testzip() is None
        assert set(archive.namelist()) == set(entries)
        for name, data in entries.items():
            assert archive.read(name) == data, name
    digest = sha(output.read_bytes())
    output.with_suffix(".zip.sha256").write_text(f"{digest}  {output.name}\n", encoding="utf-8")
    return {"file": output.name, "bytes": output.stat().st_size, "sha256": digest, "entries": len(entries), "readbackVerified": True}


def main():
    RELEASES.mkdir(exist_ok=True)
    evidence = json.loads((ROOT / "design/swiftos-v3-wallpaper-build24/candidate.json").read_text(encoding="utf-8-sig"))
    device = json.loads((ROOT / "design/swiftos-v3-wallpaper-build24/final-TV-device-validation.json").read_text())
    assert evidence["VersionCode"] == 24 and evidence["Tests"] == 257 and evidence["LintCompleted"] and evidence["R8"]
    assert evidence["SignerSHA256"] == "e0e52087919f09bb6bfe5d5fc7e986b643a2bf1f3d49b40e8827f87e3ac5f986"
    assert device["installedApkSha256"] == APKS["SwiftOS-3.0.1-build24.apk"][1]
    apk_data = {}
    for name, (source, expected) in APKS.items():
        data = (ROOT / source).read_bytes()
        assert sha(data) == expected, f"Retained APK identity changed: {name}"
        apk_data[name] = data

    tv = dict(apk_data)
    tv["README.md"] = (
        "# SwiftOS v3 local wallpaper update\n\n"
        "Build 24 is the new local-wallpaper update, already installed and smoke-tested on one Android TV. "
        "Build 23 is the unchanged previously tested v3 release; build 22 is the unchanged SwiftOS 2.7 release. "
        "Only one version can be installed because they share an app identity. Compatible updates preserve preferences. "
        "Ordinary Android downgrades are blocked; uninstalling can lose private preferences.\n\n"
        "This is a TV-only pack, with no Windows executable. The updated Windows app still needs native compilation and live integration testing. "
        "Existing Windows portable ZIPs do not contain the latest GUI changes.\n\n"
        "See WALLPAPER-ENGINE.md for photos, folders, resource limits and measured cost. Auto wallpaper was restored to Off after testing. "
        "The existing weather location is Kolkata. Other TVs, Android variants and webOS are not validated for SwiftOS.\n"
    ).encode()
    tv["WALLPAPER-ENGINE.md"] = (ROOT / "docs/WALLPAPER-ENGINE.md").read_bytes()
    tv["CleanYourTV-LICENSE.txt"] = (ROOT / ".tools/upstream/cleanyourtv-2.7.0/LICENSE").read_bytes()
    tv["Inter-OFL.txt"] = (ROOT / ".tools/swiftos-v3-source/FONT-LICENSE-OFL.txt").read_bytes()
    tv["validation.json"] = json.dumps(device, indent=2).encode()
    tv_result = write_zip("SwiftOS-v3-TV-build24", tv)

    source = {}
    for name in ["README.md", "LICENSE", "package.json", "pnpm-lock.yaml", "pnpm-workspace.yaml", "index.html", "tsconfig.json", "vite.config.ts", "vitest.config.ts", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock", "src-tauri/build.rs", "src-tauri/tauri.conf.json"]:
        source[name] = (ROOT / name).read_bytes()
    for directory in ["src", "src-tauri/src", "src-tauri/capabilities", "src-tauri/icons", "profiles", "scripts", "docs", "dist"]:
        add_tree(source, directory)
    for name, (file, _) in APKS.items():
        source[file] = apk_data[name]
    source["launchers/legacy-liquid-glass-21.apk"] = (ROOT / "launchers/legacy-liquid-glass-21.apk").read_bytes()
    for name in ["candidate.json", "final-TV-device-validation.json", "test-summary.json"]:
        file = f"design/swiftos-v3-wallpaper-build24/{name}"
        source[file] = (ROOT / file).read_bytes()
    add_tree(source, ".tools/swiftos-v3-source/app/src", "launcher-source/app/src")
    for name in ["build.gradle", "settings.gradle", "gradle.properties", "FONT-LICENSE-OFL.txt", "app/build.gradle", "app/proguard-rules.pro", "app/lint-baseline.xml"]:
        source[f"launcher-source/{name}"] = (ROOT / ".tools/swiftos-v3-source" / name).read_bytes()
    source["launcher-source/LICENSE"] = tv["CleanYourTV-LICENSE.txt"]
    source["SOURCE-UPDATE-STATUS.md"] = (
        "# Source update — not a portable Windows release\n\n"
        "The GUI passes 23 tests, TypeScript/Vite build, and isolated mock-bridge browser checks. "
        "Android build 24 passed 257 JVM tests, lint, R8 and signer checks, and bounded Mi TV wallpaper tests. "
        "New Rust code is NOT compiled or executed: Windows Application Control blocks Cargo (error 4551). "
        "No old executable is included or represented as updated. No security policy was changed.\n\n"
        "On an approved Windows development environment, prepare dependencies using scripts/Prepare-Toolchain.ps1, "
        "run scripts/Build-App.ps1 -Test -SwiftOSV3 and -CheckOnly -SwiftOSV3, then -Release -SwiftOSV3. "
        "Test the actual native GUI against a TV before running scripts/Package-Portable.ps1 -SwiftOSV3Candidate. "
        "Receipt and APK-hash gates must pass. The final end-user package still needs ADB and fixed WebView2 runtime files; this source archive does not include those development caches or runtime bundles.\n\n"
        "launcher-source contains corresponding Android source, assets, tests and Gradle configuration. "
        "Build it with an appropriate Android SDK/JDK/Gradle environment. Private signing keys and local.properties are intentionally absent. "
        "A different signing key cannot update the installed SwiftOS package. Do not replace the reviewed APK bytes or force an uninstall to bypass that restriction.\n\n"
        "No user histories, TV screenshots, photo collections, private launcher preferences or signing keys are included. "
        "See docs/UPDATE-2026-10-27.md for limits and remaining verification.\n"
    ).encode()
    source_result = write_zip("TV-Optimizer-source-update-2026-10-28", source)
    receipt = {"tv": tv_result, "source": source_result, "windowsExecutableRebuilt": False, "nativeChecks": "Blocked by Windows Application Control; new native tests are unexecuted"}
    (ROOT / "design/swiftos-v3-wallpaper-build24/release-receipt.json").write_text(json.dumps(receipt, indent=2)+"\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
