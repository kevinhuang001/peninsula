; 灵动岛 (island-lab) — per-user Windows installer.
; Build from the repository root: makensis /DAPP_VERSION=0.1.0 scripts/installer.nsi
; The CI workflow passes absolute paths so the script also works from anywhere.

Unicode true
ManifestDPIAware true
SetCompressor /SOLID lzma

!include "MUI2.nsh"
!include "FileFunc.nsh"

!ifndef APP_NAME
  !define APP_NAME "灵动岛"
!endif
!ifndef APP_VERSION
  !define APP_VERSION "0.1.0"
!endif
!ifndef APP_VERSION_QUAD
  !define APP_VERSION_QUAD "0.1.0.0"
!endif
!ifndef APP_EXE_PATH
  !define APP_EXE_PATH "..\target\release\island-lab.exe"
!endif
!ifndef ICON_PATH
  !define ICON_PATH "..\assets\icons\app.ico"
!endif
!ifndef OUT_DIR
  !define OUT_DIR "..\dist"
!endif

!define APP_EXE "island-lab.exe"
!define APP_ID "Peninsula.IslandLab"
!define APP_PUBLISHER "Kevin Wong"

Name "${APP_NAME} ${APP_VERSION}"
OutFile "${OUT_DIR}\Peninsula-${APP_VERSION}-windows-x64-setup.exe"
InstallDir "$LOCALAPPDATA\Programs\Peninsula"
InstallDirRegKey HKCU "Software\${APP_ID}" "InstallDir"
RequestExecutionLevel user

VIProductVersion "${APP_VERSION_QUAD}"
VIAddVersionKey "ProductName" "${APP_NAME}"
VIAddVersionKey "ProductVersion" "${APP_VERSION}"
VIAddVersionKey "FileDescription" "${APP_NAME} desktop capsule"
VIAddVersionKey "FileVersion" "${APP_VERSION_QUAD}"
VIAddVersionKey "CompanyName" "${APP_PUBLISHER}"
VIAddVersionKey "LegalCopyright" "© 2026 ${APP_PUBLISHER}"

!define MUI_ABORTWARNING
!define MUI_ICON "${ICON_PATH}"
!define MUI_UNICON "${ICON_PATH}"
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APP_EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "运行 ${APP_NAME}"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

LangString DESC_Main ${LANG_SIMPCHINESE} "主程序（必需）"
LangString DESC_Shortcut ${LANG_SIMPCHINESE} "在桌面创建快捷方式"
LangString DESC_Main ${LANG_ENGLISH} "Main application (required)"
LangString DESC_Shortcut ${LANG_ENGLISH} "Create a desktop shortcut"

Section "!${APP_NAME}" SecMain
  SectionIn RO
  SetOutPath "$INSTDIR"
  File "${APP_EXE_PATH}"
  File /oname=app.ico "${ICON_PATH}"

  WriteUninstaller "$INSTDIR\uninstall.exe"

  CreateDirectory "$SMPROGRAMS\${APP_NAME}"
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}" "" "$INSTDIR\app.ico" 0
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\卸载 ${APP_NAME}.lnk" "$INSTDIR\uninstall.exe"

  WriteRegStr HKCU "Software\${APP_ID}" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\${APP_ID}" "Version" "${APP_VERSION}"

  !define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "${APP_PUBLISHER}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\app.ico"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${UNINST_KEY}" "EstimatedSize" "$0"
SectionEnd

Section "桌面快捷方式" SecShortcut
  CreateShortcut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}" "" "$INSTDIR\app.ico" 0
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecMain} $(DESC_Main)
  !insertmacro MUI_DESCRIPTION_TEXT ${SecShortcut} $(DESC_Shortcut)
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  Delete "$DESKTOP\${APP_NAME}.lnk"
  Delete "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk"
  Delete "$SMPROGRAMS\${APP_NAME}\卸载 ${APP_NAME}.lnk"
  RMDir "$SMPROGRAMS\${APP_NAME}"
  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\app.ico"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}"
  DeleteRegKey HKCU "Software\${APP_ID}"
SectionEnd
