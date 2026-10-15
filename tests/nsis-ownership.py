from pathlib import Path
import os
import subprocess

root = Path(__file__).resolve().parent.parent
hooks = (root / 'src-tauri/windows/hooks.nsh').read_text(encoding='utf-8')
guard = next(line.strip() for line in hooks.splitlines() if 'StrCpy $R1' in line and 'Everything.exe' in line)
folder = root / '.tools/nsis-ownership'
folder.mkdir(parents=True, exist_ok=True)
exe = folder / 'check.exe'
nsi = folder / 'check.nsi'
# 由实际归属检查语句生成小程序，在真实 NSIS 运行时检查含空格路径。
nsi.write_text(rf'''Unicode true
!include "LogicLib.nsh"
Name "SuperSearch ownership regression"
OutFile "{exe}"
RequestExecutionLevel user
SilentInstall silent
Section
  StrCpy $INSTDIR "C:\Program Files\SuperSearch"
  StrCpy $R0 '"C:\Program Files\SuperSearch\engine\Everything.exe" -svc -instance "SuperSearch"'
  {guard}
  StrLen $R2 $R1
  StrCpy $R2 $R0 $R2
  ${{If}} $R2 != $R1
    SetErrorLevel 1
    Quit
  ${{EndIf}}
  StrCpy $R0 '"C:\Program Files\Another App\engine\Everything.exe" -svc'
  StrLen $R2 $R1
  StrCpy $R2 $R0 $R2
  ${{If}} $R2 == $R1
    SetErrorLevel 2
    Quit
  ${{EndIf}}
  SetErrorLevel 0
SectionEnd
''', encoding='utf-8')
compiler = Path(os.environ['LOCALAPPDATA']) / 'tauri/NSIS/makensis.exe'
subprocess.run([str(compiler), '/V1', str(nsi)], check=True)
result = subprocess.run([str(exe)], cwd=folder)
assert result.returncode == 0, f'NSIS ownership check failed: {result.returncode}'
print('NSIS owner path accepted; foreign owner rejected.')
