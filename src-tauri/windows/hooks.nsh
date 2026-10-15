!include "LogicLib.nsh"

; 服务只能从固定 Program Files 目录运行，拒绝任意可写安装位置。
!macro SS_CHECK_DIRECTORY
  ${If} $INSTDIR != "$PROGRAMFILES64\SuperSearch"
    MessageBox MB_ICONSTOP|MB_OK "Fast indexing must be installed in Program Files\SuperSearch."
    Abort
  ${EndIf}
!macroend

; 删除前核验 SCM 实际可执行文件归属，不触碰默认 Everything 服务。
!macro SS_REMOVE_OWN_SERVICE
  Push $R0
  Push $R1
  Push $R2
  Push $R3
  Push $R4
  Push $R5
  Push $R6
  ReadRegStr $R0 HKLM "SYSTEM\CurrentControlSet\Services\Everything (SuperSearch)" "ImagePath"
  ${If} $R0 != ""
    StrCpy $R1 '"$INSTDIR\engine\Everything.exe"'
    StrLen $R2 $R1
    StrCpy $R2 $R0 $R2
    ${If} $R2 != $R1
      MessageBox MB_ICONSTOP|MB_OK "The SuperSearch indexing service belongs to another installation. No service was changed."
      Abort
    ${EndIf}
    System::Call 'advapi32::OpenSCManagerW(p 0, p 0, i 1) p.R0'
    System::Call 'advapi32::OpenServiceW(p R0, w "Everything (SuperSearch)", i 0x10024) p.R1'
    System::Alloc 28
    Pop $R2
    StrCpy $R6 0
    ${If} $R0 != 0
    ${AndIf} $R1 != 0
    ${AndIf} $R2 != 0
      System::Call 'advapi32::ControlService(p R1, i 1, p R2) i.R3'
      ; 停止可能已在进行或已经停止；以 SCM 最终状态为准。
      StrCpy $R5 0
      ${Do}
        System::Call 'advapi32::QueryServiceStatus(p R1, p R2) i.R3'
        ${If} $R3 == 0
          ${Break}
        ${EndIf}
        System::Call '*$R2(i, i .R4)'
        ${If} $R4 == 1
          System::Call 'advapi32::DeleteService(p R1) i.R6'
          ${Break}
        ${EndIf}
        IntOp $R5 $R5 + 1
        ${If} $R5 >= 40
          ${Break}
        ${EndIf}
        Sleep 250
      ${Loop}
    ${EndIf}
    ${If} $R1 != 0
      System::Call 'advapi32::CloseServiceHandle(p R1)'
    ${EndIf}
    ${If} $R0 != 0
      System::Call 'advapi32::CloseServiceHandle(p R0)'
    ${EndIf}
    System::Free $R2
    ${If} $R6 == 0
      MessageBox MB_ICONSTOP|MB_OK "Unable to stop and remove the SuperSearch indexing service. Setup has stopped."
      Abort
    ${EndIf}
    ; DeleteService 只标记删除，注册真正消失后才能安全重装。
    StrCpy $R5 0
    ${Do}
      ReadRegStr $R0 HKLM "SYSTEM\CurrentControlSet\Services\Everything (SuperSearch)" "ImagePath"
      ${If} $R0 == ""
        ${Break}
      ${EndIf}
      IntOp $R5 $R5 + 1
      ${If} $R5 >= 40
        MessageBox MB_ICONSTOP|MB_OK "The indexing service is still pending deletion. Close service-management tools and retry."
        Abort
      ${EndIf}
      Sleep 250
    ${Loop}
  ${EndIf}
  Pop $R6
  Pop $R5
  Pop $R4
  Pop $R3
  Pop $R2
  Pop $R1
  Pop $R0
!macroend

!macro NSIS_HOOK_PREINSTALL
  !insertmacro SS_CHECK_DIRECTORY
  ; 所有可取消的应用退出提示必须发生在服务删除之前。
  !insertmacro CheckIfAppIsRunning "$INSTDIR\${MAINBINARYNAME}.exe" "${PRODUCTNAME}"
  !insertmacro SS_REMOVE_OWN_SERVICE
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; 原厂签名 EXE 自行安装专属实例，并将管道限定为 SuperSearch。
  ExecWait '"$INSTDIR\engine\Everything.exe" -instance SuperSearch -install-service -install-service-pipe-name "\\.\PIPE\SuperSearch Everything Service"' $R0
  ; 等待原厂安装过程完成注册，不能把启动成功当作服务已经存在。
  StrCpy $R2 0
  ${Do}
    ReadRegStr $R1 HKLM "SYSTEM\CurrentControlSet\Services\Everything (SuperSearch)" "ImagePath"
    ${If} $R1 != ""
      ${Break}
    ${EndIf}
    IntOp $R2 $R2 + 1
    ${If} $R2 >= 40
      ${Break}
    ${EndIf}
    Sleep 250
  ${Loop}
  ${If} $R0 != 0
  ${OrIf} $R1 == ""
    MessageBox MB_ICONEXCLAMATION|MB_OK "Fast indexing service installation failed. SuperSearch will use folder indexing."
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro SS_CHECK_DIRECTORY
  !insertmacro CheckIfAppIsRunning "$INSTDIR\${MAINBINARYNAME}.exe" "${PRODUCTNAME}"
  !insertmacro SS_REMOVE_OWN_SERVICE
!macroend
