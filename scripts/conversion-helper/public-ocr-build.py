"""从校验过的官方源码构建只含 PNG 输入的公开 OCR 组件。"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import urllib.request

from package import archive_component

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
WORK = ROOT / 'test-results/public-ocr-build'
DELIVERY = ROOT / 'artifacts/releases/Public-OCR-2026-10-25'


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--cmake')
    args = parser.parse_args()
    if os.name != 'nt':
        raise SystemExit('Build this component on Windows with Visual Studio 2022 C++ tools')
    cmake = args.cmake or shutil.which('cmake')
    if not cmake:
        candidates = list(Path(os.environ.get('ProgramFiles(x86)', 'C:/Program Files (x86)')).glob('Microsoft Visual Studio/*/*/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe'))
        cmake = str(sorted(candidates)[-1]) if candidates else None
    if not cmake:
        raise RuntimeError('CMake is unavailable')
    downloads, sources, prefix = WORK / 'downloads', WORK / 'sources', WORK / 'install'
    downloads.mkdir(parents=True, exist_ok=True)
    sources.mkdir(exist_ok=True)
    items = json.loads((HERE / 'public-ocr-build-sources.json').read_text())
    for item in items:
        path = downloads / item['name']
        if not path.exists():
            print('Downloading ' + item['name'], flush=True)
            with urllib.request.urlopen(item['url'], timeout=60) as response, path.open('wb') as output:
                shutil.copyfileobj(response, output, length=1024 * 1024)
        if path.stat().st_size != item['bytes'] or digest(path) != item['sha256']:
            raise RuntimeError('Source archive verification failed: ' + item['name'])
        with tarfile.open(path) as archive:
            # Python 的 data 过滤器拒绝越界路径和危险链接。
            archive.extractall(sources, filter='data')
    commands = []
    def run(command, label):
        commands.append(command)
        print(label, flush=True)
        with (WORK / (label + '.log')).open('w', encoding='utf-8') as log:
            result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, creationflags=subprocess.CREATE_NO_WINDOW)
        if result.returncode:
            raise RuntimeError(label + ' failed; see ' + str(WORK / (label + '.log')))
    common = ['-G', 'Visual Studio 17 2022', '-A', 'x64', '-DCMAKE_POLICY_DEFAULT_CMP0091=NEW', '-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded', '-DCMAKE_INSTALL_PREFIX=' + prefix.as_posix(), '-DCMAKE_PREFIX_PATH=' + prefix.as_posix()]
    zlib = prefix / 'lib/zlibstatic.lib'
    png = prefix / 'lib/libpng16_static.lib'
    pins = ['-DZLIB_LIBRARY=' + zlib.as_posix(), '-DZLIB_INCLUDE_DIR=' + (prefix/'include').as_posix()]
    configurations = [
        ('zlib-1.3.1', ['-DZLIB_BUILD_EXAMPLES=OFF']),
        ('libpng-1.6.50', pins + ['-DPNG_SHARED=OFF', '-DPNG_STATIC=ON', '-DPNG_TESTS=OFF', '-DPNG_TOOLS=OFF', '-DPNG_EXECUTABLES=OFF']),
        ('leptonica-1.85.0', pins + ['-DPNG_LIBRARY=' + png.as_posix(), '-DPNG_PNG_INCLUDE_DIR=' + (prefix/'include').as_posix(), '-DBUILD_SHARED_LIBS=OFF', '-DSW_BUILD=OFF', '-DBUILD_PROG=OFF', '-DENABLE_ZLIB=ON', '-DENABLE_PNG=ON'] + ['-DENABLE_' + name + '=OFF' for name in ['GIF','JPEG','TIFF','WEBP','OPENJPEG']]),
        ('tesseract-5.5.3', ['-DBUILD_SHARED_LIBS=ON', '-DWIN32_MT_BUILD=ON', '-DOPENMP_BUILD=OFF', '-DGRAPHICS_DISABLED=ON', '-DBUILD_TRAINING_TOOLS=OFF', '-DBUILD_TESTS=OFF', '-DDISABLE_TIFF=ON', '-DDISABLE_ARCHIVE=ON', '-DDISABLE_CURL=ON', '-DENABLE_NATIVE=OFF', '-DENABLE_CCACHE=OFF', '-DCMAKE_PROJECT_tesseract_INCLUDE=' + (WORK/'tesseract-output.cmake').as_posix()]),
    ]
    # 延迟到目标创建后固定 DLL 名称；导入库和 CLI 同步采用同一名称。
    (WORK/'tesseract-output.cmake').write_text('cmake_language(DEFER CALL set_target_properties libtesseract PROPERTIES OUTPUT_NAME "libtesseract-5")\n')
    for name, options in configurations:
        build = WORK / ('build-' + name)
        run([cmake, '-S', str(sources/name), '-B', str(build), *common, *options], name + '-configure')
        run([cmake, '--build', str(build), '--config', 'Release', '--parallel', '2'], name + '-build')
        run([cmake, '--install', str(build), '--config', 'Release'], name + '-install')
    stage = WORK/'stage'
    stage.mkdir(exist_ok=True)
    for name in ['tesseract.exe', 'libtesseract-5.dll']:
        shutil.copyfile(prefix/'bin'/name, stage/name)
    tessdata = stage/'tessdata'
    tessdata.mkdir(exist_ok=True)
    tess_sources = sources/'tesseract-5.5.3/tessdata'
    for name in ['configs','tessconfigs']:
        shutil.copytree(tess_sources/name, tessdata/name, dirs_exist_ok=True)
    shutil.copyfile(tess_sources/'pdf.ttf', tessdata/'pdf.ttf')
    model_items = [item for item in json.loads((HERE/'ocr-sources.json').read_text(encoding='utf-8-sig')) if item['name'].endswith('.traineddata')]
    for item in model_items:
        model = ROOT/'test-results/conversion-ocr-downloads'/item['name']
        if not model.exists():
            model = downloads/item['name']
            if not model.exists():
                urllib.request.urlretrieve(item['url'], model)
        if model.stat().st_size != item['bytes'] or digest(model) != item['sha256']:
            raise RuntimeError('Model verification failed: ' + item['name'])
        shutil.copyfile(model, tessdata/item['name'])
    licenses = stage/'licenses'
    licenses.mkdir(exist_ok=True)
    for source, target in [('tesseract-5.5.3/LICENSE','Tesseract-Apache-2.0.txt'), ('leptonica-1.85.0/leptonica-license.txt','Leptonica-BSD-2-Clause.txt'), ('libpng-1.6.50/LICENSE','libpng-LICENSE.txt'), ('zlib-1.3.1/README','zlib-README-and-license.txt')]:
        shutil.copyfile(sources/source, licenses/target)
    shutil.copyfile(sources/'tesseract-5.5.3/LICENSE', licenses/'tessdata-fast-Apache-2.0.txt')
    component = {'id':'ocr','version':'1.0.1','entry':'tesseract.exe','engineVersion':'5.5.3','source':'https://github.com/tesseract-ocr/tesseract/releases/tag/5.5.3','license':'Apache-2.0 (Tesseract and models); BSD-2-Clause (Leptonica); libpng-2.0; Zlib'}
    provenance = {'buildMethod':'Built from pinned official source archives with MSVC; static Leptonica/libpng/zlib and static CRT', 'inputFiles':items+model_items, 'commands':commands, 'runtimeFiles':[{'name':name, 'bytes':(stage/name).stat().st_size,'sha256':digest(stage/name)} for name in ['tesseract.exe','libtesseract-5.dll']], 'features':{'imageInputs':['PNG','BMP','PNM'], 'openmp':False,'archive':False,'curl':False,'training':False,'hostNativeOptimization':False,'legacyEngine':True}, 'publicationStatus':'Redistribution includes original upstream licenses and source/build provenance. No extracted installer runtime files are included.'}
    (stage/'component.json').write_text(json.dumps(component,indent=2)+'\n')
    (stage/'PROVENANCE.json').write_text(json.dumps(provenance,indent=2)+'\n')
    shutil.copyfile(HERE/'OCR-NOTICES.md', stage/'OCR-NOTICES.md')
    shutil.copyfile(HERE/'public-ocr-build-sources.json', stage/'SOURCE-ARCHIVES.json')
    probe = subprocess.run([str(stage/'tesseract.exe'),'--version'],capture_output=True,text=True,check=True,creationflags=subprocess.CREATE_NO_WINDOW)
    if 'tesseract 5.5.3' not in probe.stdout:
        raise RuntimeError('Unexpected engine version')
    (WORK/'version.txt').write_text(probe.stdout+probe.stderr)
    DELIVERY.mkdir(parents=True,exist_ok=True)
    details=archive_component(stage, DELIVERY/'supersearch-ocr-component-1.0.1-windows-x64.zip', component)
    (WORK/'build-result.json').write_text(json.dumps(details,indent=2)+'\n')
    source_stage = WORK/'source-package'
    for item in items:
        destination = source_stage/'test-results/public-ocr-build/downloads'/item['name']
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(downloads/item['name'], destination)
    source_scripts = source_stage/'scripts/conversion-helper'
    source_scripts.mkdir(parents=True, exist_ok=True)
    for name in ['public-ocr-build.py','public-ocr-build-verify.py','public-ocr-build-sources.json','package.py','package_ocr.py','ocr-sources.json','OCR-NOTICES.md']:
        shutil.copyfile(HERE/name, source_scripts/name)
    shutil.copytree(licenses, source_stage/'licenses', dirs_exist_ok=True)
    shutil.copyfile(stage/'PROVENANCE.json', source_stage/'PROVENANCE.json')
    (source_stage/'README.txt').write_text('Official upstream archives and SuperSearch build scripts for OCR component 1.0.1.\nExtract this archive into an empty directory. With Python 3.12+ and Visual Studio 2022 C++/CMake tools, run:\npython scripts/conversion-helper/public-ocr-build.py\nThe two pinned language models are fetched from official upstream URLs if absent.\nTo verify, install Pillow and pefile in a build-only Python environment, then run:\npython scripts/conversion-helper/public-ocr-build-verify.py\nNo upstream application installer is executed.\n')
    source_details = archive_component(source_stage, DELIVERY/'supersearch-ocr-sources-1.0.1.zip', {'id':'ocr-sources','version':'1.0.1','source':component['source'],'license':component['license']})
    print(json.dumps({'component':details,'sources':source_details},indent=2))


if __name__ == '__main__':
    main()
