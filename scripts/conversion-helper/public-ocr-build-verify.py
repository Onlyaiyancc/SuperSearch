"""验证公开 OCR 的依赖边界、CLI 和真实 C API 识别。"""

import ctypes
import hashlib
import json
from pathlib import Path
import subprocess

import pefile
from PIL import Image, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
WORK = ROOT/'test-results/public-ocr-build'


def main():
    stage = WORK/'stage'
    native = {}
    for name in ['tesseract.exe', 'libtesseract-5.dll']:
        pe = pefile.PE(str(stage/name))
        imports = [entry.dll.decode('ascii') for entry in getattr(pe,'DIRECTORY_ENTRY_IMPORT',[])]
        pe.close()
        # 白名单限制为 Windows 系统库以及本组件自己的固定 DLL。
        allowed = {'kernel32.dll','user32.dll','gdi32.dll','advapi32.dll','shell32.dll','ws2_32.dll','ole32.dll','oleaut32.dll','comdlg32.dll','libtesseract-5.dll'}
        assert all(value.lower() in allowed or value.lower().startswith(('api-ms-win-','ext-ms-win-')) for value in imports), imports
        native[name] = {'imports':imports,'sha256':hashlib.file_digest((stage/name).open('rb'),'sha256').hexdigest()}
    api = ctypes.CDLL(str(stage/'libtesseract-5.dll'),winmode=0x900)
    api.TessBaseAPICreate.restype=ctypes.c_void_p
    for name, types in {
        'TessBaseAPIInit3':[ctypes.c_void_p,ctypes.c_char_p,ctypes.c_char_p],
        'TessBaseAPISetVariable':[ctypes.c_void_p,ctypes.c_char_p,ctypes.c_char_p],
        'TessBaseAPISetPageSegMode':[ctypes.c_void_p,ctypes.c_int],
        'TessBaseAPISetImage':[ctypes.c_void_p,ctypes.c_void_p,ctypes.c_int,ctypes.c_int,ctypes.c_int,ctypes.c_int],
        'TessBaseAPIGetTsvText':[ctypes.c_void_p,ctypes.c_int],
        'TessBaseAPIGetUTF8Text':[ctypes.c_void_p],
        'TessDeleteText':[ctypes.c_void_p],
        'TessBaseAPIDelete':[ctypes.c_void_p],
    }.items():
        getattr(api,name).argtypes=types
    api.TessBaseAPIGetTsvText.restype=ctypes.c_void_p
    api.TessBaseAPIGetUTF8Text.restype=ctypes.c_void_p
    engine=api.TessBaseAPICreate()
    assert api.TessBaseAPIInit3(engine,str(stage/'tessdata').encode(),b'eng+chi_sim')==0
    api.TessBaseAPISetVariable(engine,b'debug_file',b'NUL')
    api.TessBaseAPISetPageSegMode(engine,11)
    cases=[]
    try:
        for font in ['segoeui.ttf','arial.ttf','msyh.ttc']:
            for size in [12,14,16,18,20,24]:
                for value in ['https://tjch.hotjob.cn','https://tich.hotjob.cn']:
                    image=Image.new('L',(440,80),255)
                    ImageDraw.Draw(image).text((18,20),value,font=ImageFont.truetype('C:/Windows/Fonts/'+font,size),fill=0)
                    outputs={}
                    for scale in [1,3]:
                        sample=image if scale==1 else image.resize((image.width*scale,image.height*scale),Image.Resampling.LANCZOS)
                        pixels=ctypes.create_string_buffer(sample.tobytes())
                        api.TessBaseAPISetImage(engine,pixels,sample.width,sample.height,1,sample.width)
                        pointer=api.TessBaseAPIGetUTF8Text(engine)
                        try:outputs[str(scale)]=ctypes.string_at(pointer).decode('utf-8').strip()
                        finally:api.TessDeleteText(pointer)
                    pointer=api.TessBaseAPIGetTsvText(engine,0)
                    try:tsv=ctypes.string_at(pointer).decode('utf-8')
                    finally:api.TessDeleteText(pointer)
                    assert len(tsv.splitlines()) > 1 and '\t' in tsv
                    cases.append({'font':font,'size':size,'expected':value,'outputs':outputs})
                    if font=='segoeui.ttf' and size==12 and 'tjch' in value:
                        sample.save(WORK/'tjch-3x.png')
                        (WORK/'tjch-3x.tsv').write_text(tsv,encoding='utf-8')
                        assert outputs['3'].replace(' ','')==value,outputs
    finally:
        api.TessBaseAPIDelete(engine)
    command=[str(stage/'tesseract.exe'),str(WORK/'tjch-3x.png'),'stdout','--tessdata-dir',str(stage/'tessdata'),'-l','eng+chi_sim','--psm','11']
    cli=subprocess.run(command,capture_output=True,text=True,check=True,timeout=30,creationflags=subprocess.CREATE_NO_WINDOW)
    assert cli.stdout.strip().replace(' ','')=='https://tjch.hotjob.cn',cli.stdout
    pdf_command=[str(stage/'tesseract.exe'),str(WORK/'tjch-3x.png'),str(WORK/'searchable'),'--tessdata-dir',str(stage/'tessdata'),'-l','eng+chi_sim','--psm','11','pdf']
    subprocess.run(pdf_command,capture_output=True,check=True,timeout=30,creationflags=subprocess.CREATE_NO_WINDOW)
    assert (WORK/'searchable.pdf').read_bytes().startswith(b'%PDF-')
    report={'syntheticOnly':True,'native':native,'cases':cases,'correctByScale':{str(scale):sum(row['outputs'][str(scale)].replace(' ','')==row['expected'] for row in cases) for scale in [1,3]},'cliText':cli.stdout.strip(),'tsvCApi':True,'searchablePdf':True}
    (WORK/'verification.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({key:value for key,value in report.items() if key!='cases'},indent=2))


if __name__=='__main__':
    main()
