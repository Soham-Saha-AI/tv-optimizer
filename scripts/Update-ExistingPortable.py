"""Update the workspace's existing portable copy without packaging its data."""
from pathlib import Path
from datetime import datetime, timezone
import hashlib
import json
import shutil
import sys
import zipfile

ROOT = Path(__file__).resolve().parents[1]
DESTINATION = ROOT / "releases/TV-Optimizer-0.1.0-Windows-x64/TVOptimizer"


def digest(file):
    with file.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def contained(root, name):
    path = (root / name).resolve()
    if not path.is_relative_to(root.resolve()) or path == root.resolve():
        raise RuntimeError(f"Update path escapes its folder: {name}")
    return path


def data_state():
    # Recovery databases are verified without opening or exporting preferences.
    # WebView's live GPU cache can be locked and is never part of this update.
    return {p.relative_to(DESTINATION).as_posix(): digest(p)
            for p in sorted((DESTINATION / "data").glob("history.sqlite3*")) if p.is_file()}


def main():
    stage = Path(sys.argv[1]).resolve()
    assert stage.is_relative_to((ROOT / ".tools/package-staging").resolve())
    manifest_path = stage / "integrity-manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8-sig"))
    receipt = json.loads((ROOT / ".tools/last-build-receipt.json").read_text(encoding="utf-8-sig"))
    assert receipt["status"] == "success" and receipt["launcher_version_code"] == 24
    assert digest(stage / "TV Optimizer.exe") == receipt["sha256"]
    for record in manifest["files"]:
        file = contained(stage, record["path"])
        assert file.stat().st_size == record["bytes"] and digest(file) == record["sha256"], record["path"]
    stamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    backup = ROOT / ".tools/portable-update-backups" / stamp
    backup.mkdir(parents=True, exist_ok=False)
    before = data_state()
    changed = []
    for record in manifest["files"]:
        name = record["path"]
        if name.startswith("data/"):
            continue
        source, destination = contained(stage, name), contained(DESTINATION, name)
        if destination.is_file() and digest(destination) == record["sha256"]:
            continue
        if destination.exists():
            saved = contained(backup / "application", name)
            saved.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(destination, saved)
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
        assert digest(destination) == record["sha256"], name
        changed.append(name)
    prior_manifest = DESTINATION / "integrity-manifest.json"
    shutil.copy2(prior_manifest, backup / "previous-integrity-manifest.json")
    installed_manifest = dict(manifest)
    installed_manifest["files"] = [r for r in manifest["files"] if not r["path"].startswith("data/")]
    prior_manifest.write_text(json.dumps(installed_manifest, indent=2)+"\n", encoding="utf-8")
    assert data_state() == before, "The existing portable data directory changed during update"
    result = {"destination": str(DESTINATION), "applicationBackup": str(backup), "changedFiles": len(changed),
              "recoveryFilesHashVerified": len(before), "recoveryHashesUnchanged": True, "dataDirectoryExcludedFromUpdate": True, "executableSha256": receipt["sha256"], "archives": []}
    print(json.dumps({"portableFolderUpdated": result}), flush=True)
    # ZIPs are built from the fresh manifest allowlist, never the owner's folder.
    for label, prefix in [("TV-Optimizer-0.1.0-Windows-x64", "TVOptimizer"), ("TV-Optimizer-0.1.0-SwiftOS-v3-Windows-x64", "TVOptimizer-SwiftOS-v3")]:
        output = ROOT / "releases" / f"{label}.zip"
        pending = output.with_suffix(".zip.pending")
        with zipfile.ZipFile(pending, "w", zipfile.ZIP_DEFLATED, compresslevel=6, strict_timestamps=False) as archive:
            for record in manifest["files"]:
                archive.write(contained(stage, record["path"]), f"{prefix}/{record['path']}")
            archive.write(manifest_path, f"{prefix}/integrity-manifest.json")
        with zipfile.ZipFile(pending) as archive:
            assert archive.testzip() is None
            for record in manifest["files"]:
                with archive.open(f"{prefix}/{record['path']}") as stream:
                    assert hashlib.file_digest(stream,"sha256").hexdigest() == record["sha256"]
            assert all(not n.startswith(f"{prefix}/data/") or n == f"{prefix}/data/README.txt" for n in archive.namelist())
        old = output.with_name(f"{label}.previous-{stamp}.zip")
        if output.exists():
            # Both fully resolved paths are explicitly inside this releases folder.
            assert output.resolve().parent == old.resolve().parent == (ROOT / "releases").resolve()
            output.rename(old)
            old_checksum = output.with_suffix(".zip.sha256")
            if old_checksum.exists():
                old_checksum.rename(old.with_suffix(".zip.sha256"))
        pending.replace(output)
        checksum = digest(output)
        output.with_suffix(".zip.sha256").write_text(f"{checksum}  {output.name}\n")
        result["archives"].append({"file": str(output), "bytes": output.stat().st_size, "sha256": checksum, "readbackVerified": True, "previous": str(old)})
        print(json.dumps({"archiveUpdated": result["archives"][-1]}), flush=True)
    result["completedUtc"] = datetime.now(timezone.utc).isoformat()
    (ROOT / "design/swiftos-v3-wallpaper-build24/portable-update-receipt.json").write_text(json.dumps(result, indent=2)+"\n")
    print(json.dumps({"result": "passed", **result}), flush=True)


if __name__ == "__main__":
    main()
