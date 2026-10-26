"""构建独立 Windows helper，并输出供原生安装器固定信任的校验清单。"""

import hashlib
from importlib import metadata
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import zipfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
VERSION = "1.0.0"
EPOCH = "1767225600"


def archive_component(stage, destination, component):
    files = sorted(path for path in stage.rglob("*") if path.is_file())
    with zipfile.ZipFile(destination, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in files:
            entry = zipfile.ZipInfo(path.relative_to(stage).as_posix(), date_time=(2026, 1, 1, 0, 0, 0))
            entry.compress_type = zipfile.ZIP_DEFLATED
            entry.external_attr = 0o100644 << 16
            archive.writestr(entry, path.read_bytes(), compresslevel=9)
    details = {**component, "archive": destination.name, "sha256": hashlib.file_digest(destination.open("rb"), "sha256").hexdigest(), "downloadBytes": destination.stat().st_size, "installedBytes": sum(path.stat().st_size for path in files), "unpackedLimit": sum(path.stat().st_size for path in files) + 1024 * 1024}
    destination.with_suffix(".json").write_text(json.dumps(details, indent=2) + "\n", encoding="utf-8")
    return details


def copy_licenses(stage):
    licenses = stage / "licenses"
    licenses.mkdir(exist_ok=True)
    dependencies = []
    for distribution in sorted(metadata.distributions(), key=lambda item: item.metadata["Name"].lower()):
        name, version = distribution.metadata["Name"], distribution.version
        if name.lower() in {"pip", "setuptools", "pyinstaller-hooks-contrib", "altgraph", "pefile", "pywin32-ctypes", "packaging"}:
            continue
        entries = []
        for item in distribution.files or []:
            parts = str(item).replace("\\", "/").split("/")
            if any(part.lower().startswith(("license", "licence", "copying", "notice", "copyright")) for part in parts):
                source = Path(distribution.locate_file(item))
                if source.is_file() and source.stat().st_size < 10 * 1024 * 1024:
                    destination = licenses / name / "/".join(parts[1:])
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(source, destination)
                    entries.append(destination.relative_to(stage).as_posix())
        if not entries:
            raise RuntimeError(f"Missing distributed license for {name} {version}")
        dependencies.append({"name": name, "version": version, "license": distribution.metadata.get("License-Expression") or distribution.metadata.get("License") or "See distributed license files", "projectUrls": distribution.metadata.get_all("Project-URL") or [], "licenseFiles": entries})
    base_license = Path(sys.base_prefix) / "LICENSE.txt"
    if not base_license.is_file():
        base_license = Path(sys.base_prefix) / "LICENSE_PYTHON.txt"
    if not base_license.is_file():
        raise RuntimeError("Python runtime license is unavailable")
    shutil.copyfile(base_license, licenses / "Python-LICENSE.txt")
    (stage / "DEPENDENCIES.json").write_text(json.dumps(dependencies, indent=2) + "\n", encoding="utf-8")
    # 原生 DLL 必须匹配已审查的来源及许可快照，环境漂移时拒绝打包。
    if (stage / "_internal").is_dir():
        import runpy
        collector = runpy.run_path(str(HERE / "public-pdf-notices.py"))
        collector["copy_native_licenses"](stage)


def main():
    if os.name != "nt":
        raise SystemExit("Build the Windows conversion component on Windows")
    build = ROOT / "test-results" / "conversion-helper-package"
    delivery = ROOT / "artifacts" / "releases" / "Format-Conversion-2026-10-21"
    build.mkdir(parents=True, exist_ok=True)
    delivery.mkdir(parents=True, exist_ok=True)
    environment = dict(os.environ, SOURCE_DATE_EPOCH=EPOCH)
    command = [sys.executable, "-m", "PyInstaller", "--noconfirm", "--clean", "--onedir", "--console", "--name", "supersearch-conversion-helper", "--distpath", str(build / "dist"), "--workpath", str(build / "work"), "--specpath", str(build), "--collect-data", "docx", "--collect-data", "pptx", "--collect-all", "pypdfium2", "--collect-all", "pypdfium2_raw", str(HERE / "helper.py")]
    subprocess.run(command, check=True, env=environment, creationflags=subprocess.CREATE_NO_WINDOW)
    stage = build / "dist" / "supersearch-conversion-helper"
    component = {"id": "pdf", "version": VERSION, "entry": "supersearch-conversion-helper.exe", "source": "https://github.com/Onlyaiyancc/SuperSearch/tree/main/scripts/conversion-helper", "license": "MIT; bundled libraries retain their individual licenses"}
    (stage / "component.json").write_text(json.dumps(component, indent=2) + "\n", encoding="utf-8")
    shutil.copyfile(HERE / "helper.py", stage / "helper-source.py")
    shutil.copyfile(HERE / "requirements.lock", stage / "requirements.lock")
    shutil.copyfile(ROOT / "LICENSE", stage / "LICENSE.txt")
    copy_licenses(stage)
    probe = subprocess.run([str(stage / component["entry"]), "--version"], check=True, capture_output=True, text=True, timeout=30, creationflags=subprocess.CREATE_NO_WINDOW)
    if VERSION not in probe.stdout:
        raise RuntimeError("Packaged helper probe returned an unexpected version")
    details = archive_component(stage, delivery / f"supersearch-pdf-component-{VERSION}-windows-x64.zip", component)
    print(json.dumps(details, indent=2))


if __name__ == "__main__":
    main()
