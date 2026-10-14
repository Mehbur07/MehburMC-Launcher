; Keeps the per-user install in %APPDATA% (Roaming) next to the data folder
; instead of the NSIS default %LOCALAPPDATA% (ARCHITECTURE.md K79).

Var MehburOldInstDir

!macro NSIS_HOOK_PREINSTALL
  StrCpy $MehburOldInstDir ""
  ; Only the default location moves; a folder the user picked is kept.
  ${If} $INSTDIR == "$LOCALAPPDATA\${PRODUCTNAME}"
    StrCpy $MehburOldInstDir $INSTDIR
    ; Tauri checks only the new path; close a copy still running from the old one.
    !insertmacro CheckIfAppIsRunning "$MehburOldInstDir\${MAINBINARYNAME}.exe" "${PRODUCTNAME}"
    StrCpy $INSTDIR "$APPDATA\MehburMC\app"
    SetOutPath $INSTDIR
  ${EndIf}
!macroend

!macro MehburRetarget lnk
  !insertmacro IsShortcutTarget "${lnk}" "$MehburOldInstDir\${MAINBINARYNAME}.exe"
  Pop $0
  ${If} $0 = 1
    !insertmacro SetShortcutTarget "${lnk}" "$INSTDIR\${MAINBINARYNAME}.exe"
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ${If} $MehburOldInstDir != ""
    ; Updates keep existing shortcuts; point them at the new exe.
    !insertmacro MehburRetarget "$DESKTOP\${PRODUCTNAME}.lnk"
    !insertmacro MehburRetarget "$SMPROGRAMS\${PRODUCTNAME}.lnk"
    !insertmacro MehburRetarget "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk"
    Delete "$MehburOldInstDir\${MAINBINARYNAME}.exe"
    Delete "$MehburOldInstDir\uninstall.exe"
    RMDir "$MehburOldInstDir"
  ${EndIf}
!macroend
