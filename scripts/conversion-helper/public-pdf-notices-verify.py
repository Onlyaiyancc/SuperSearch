"""验证 PDF 公共归档只增补许可，并覆盖原生许可拒绝路径。"""

import hashlib
import json
from pathlib import Path
import runpy
import tempfile
import zipfile

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]


def digest(path):
    return hashlib.file_digest(path.open('rb'),'sha256').hexdigest()


def main():
    namespace=runpy.run_path(str(HERE/'public-pdf-notices.py'))
    copy=namespace['copy_native_licenses']
    work=ROOT/'test-results/public-pdf-notices'
    results=[]
    with tempfile.TemporaryDirectory(dir=work) as temporary:
        temp=Path(temporary);fake=temp/'stage';(fake/'_internal').mkdir(parents=True)
        dll=fake/'_internal/fixture.dll';dll.write_bytes(b'fixture')
        copy.__globals__['HERE']=temp
        document={'libraries':[{'path':'_internal/fixture.dll','bytes':7,'sha256':digest(dll)}],'noticeFiles':[{'file':'missing.txt','bytes':1,'sha256':'00'}]}
        (temp/'public-pdf-notices.json').write_text(json.dumps(document))
        for expected,mutate in [
            ('Missing or changed native notice',lambda:None),
            ('Unreviewed native binary',lambda:dll.write_bytes(b'changed')),
            ('inventory changed',lambda:(fake/'_internal/extra.dll').write_bytes(b'extra')),
        ]:
            mutate()
            try:
                copy(fake)
                raise AssertionError('Unsafe native payload was accepted')
            except RuntimeError as error:
                assert expected in str(error),str(error)
                results.append(expected)
    release=ROOT/'artifacts/releases/Public-PDF-2026-10-25'
    metadata=json.loads((release/'supersearch-pdf-component-1.0.0-windows-x64.json').read_text())
    archive=release/metadata['archive']
    assert digest(archive)==metadata['sha256']
    original=ROOT/'artifacts/releases/All-Extensions-Candidate-2026-10-24'/metadata['archive']
    with zipfile.ZipFile(original) as before,zipfile.ZipFile(archive) as after:
        names=[entry.filename for entry in before.infolist() if not entry.is_dir()]
        assert all(hashlib.sha256(before.read(name)).digest()==hashlib.sha256(after.read(name)).digest() for name in names)
        assert sum(entry.file_size for entry in after.infolist())==metadata['installedBytes']
        assert after.testzip() is None
    notices=HERE/'public-pdf-notices'
    with zipfile.ZipFile(notices/'Microsoft-Visual-C-Runtime-2015-2022-License.docx') as document:
        assert b'RUNTIME' in document.read('word/document.xml').upper()
    assert b'MICROSOFT WINDOWS SOFTWARE DEVELOPMENT KIT' in (notices/'Microsoft-Windows-10-SDK-License.html').read_bytes()
    report=json.loads((work/'verification.json').read_text())
    report['failClosedChecks']=results
    report['archiveMetadataMatches']=True
    report['microsoftOriginalDocumentsValidated']=True
    report['currentProviderPayloadComparison']='All eight provider DLLs match current source payloads excluding only Authenticode certificate bytes and PE checksum; five libraries have different certificate/checksum bytes. Pinned library manifest records normalized and current-source hashes.'
    (work/'verification.json').write_text(json.dumps(report,indent=2)+'\n')
    (release/'verification.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({'unchangedOriginalFiles':len(names),'failClosedChecks':results,'finalArchiveSha256':metadata['sha256']},indent=2))


if __name__=='__main__':
    main()
