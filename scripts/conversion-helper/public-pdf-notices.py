"""补齐公开 PDF 组件的原生许可证，并拒绝未经审查的二进制漂移。"""

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import zipfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def copy_native_licenses(stage):
    stage = Path(stage)
    manifest_path = HERE / 'public-pdf-notices.json'
    manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
    expected = {item['path'] for item in manifest['libraries']}
    actual = {path.relative_to(stage).as_posix() for path in (stage/'_internal').glob('*.dll')}
    if actual != expected:
        raise RuntimeError('Native DLL inventory changed; review sources and licenses before packaging')
    for item in manifest['libraries']:
        path = stage/item['path']
        if path.stat().st_size != item['bytes'] or digest(path) != item['sha256']:
            raise RuntimeError('Unreviewed native binary: ' + item['path'])
    target = stage/'native-licenses'
    target.mkdir(exist_ok=True)
    for item in manifest['noticeFiles']:
        path = HERE/'public-pdf-notices'/item['file']
        if not path.is_file() or path.stat().st_size != item['bytes'] or digest(path) != item['sha256']:
            raise RuntimeError('Missing or changed native notice: ' + item['file'])
        shutil.copyfile(path,target/item['file'])
    shutil.copyfile(manifest_path,target/'SOURCES.json')
    shutil.copyfile(HERE/'public-pdf-notices.md',target/'README.md')
    provenance = {'method':'Original verified PDF helper payload, with added native dependency license/source materials; no executable or library rebuilt or modified','originalArchiveSha256':manifest['originalArchiveSha256'],'nativeLicenseManifest':manifest,'noticeManifestSha256':digest(manifest_path)}
    (stage/'PROVENANCE.json').write_text(json.dumps(provenance,indent=2)+'\n',encoding='utf-8')
    return manifest


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--archive',type=Path,default=ROOT/'artifacts/releases/All-Extensions-Candidate-2026-10-24/supersearch-pdf-component-1.0.0-windows-x64.zip')
    args=parser.parse_args()
    manifest=json.loads((HERE/'public-pdf-notices.json').read_text())
    if digest(args.archive) != manifest['originalArchiveSha256']:
        raise RuntimeError('Original PDF archive hash mismatch')
    work=ROOT/'test-results/public-pdf-notices'
    stage=work/'stage'
    stage.mkdir(parents=True,exist_ok=True)
    with zipfile.ZipFile(args.archive) as archive:
        for entry in archive.infolist():
            if not (stage/entry.filename).resolve().is_relative_to(stage.resolve()):
                raise RuntimeError('Archive path escapes staging directory')
        archive.extractall(stage)
        baseline={entry.filename:hashlib.sha256(archive.read(entry)).hexdigest() for entry in archive.infolist() if not entry.is_dir()}
    copy_native_licenses(stage)
    if any(digest(stage/name)!=value for name,value in baseline.items()):
        raise RuntimeError('Original component payload changed')
    from package import archive_component
    delivery=ROOT/'artifacts/releases/Public-PDF-2026-10-25'
    delivery.mkdir(parents=True,exist_ok=True)
    component=json.loads((stage/'component.json').read_text())
    details=archive_component(stage,delivery/'supersearch-pdf-component-1.0.0-windows-x64.zip',component)
    report={'unchangedOriginalFiles':len(baseline),'allOriginalPayloadSha256Matched':True,'originalArchiveSha256':manifest['originalArchiveSha256'],'newArchive':details,'nativeDlls':len(manifest['libraries']),'nativeNotices':len(manifest['noticeFiles'])}
    for flag in ['--version','--probe']:
        result=subprocess.run([str(stage/component['entry']),flag],capture_output=True,text=True,check=True,timeout=30,creationflags=subprocess.CREATE_NO_WINDOW)
        if '1.0.0' not in result.stdout:
            raise RuntimeError('Unexpected helper version')
        report[flag]=result.stdout.strip()
    with zipfile.ZipFile(delivery/details['archive']) as archive:
        if archive.testzip() is not None:
            raise RuntimeError('Final archive CRC failed')
    (work/'verification.json').write_text(json.dumps(report,indent=2)+'\n')
    shutil.copyfile(work/'verification.json',delivery/'verification.json')
    shutil.copyfile(stage/'PROVENANCE.json',delivery/'PROVENANCE.json')
    print(json.dumps(details,indent=2))


if __name__=='__main__':
    main()
