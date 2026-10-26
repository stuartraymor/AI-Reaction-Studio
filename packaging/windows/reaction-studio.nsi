Unicode True
!include "MUI2.nsh"
!include "x64.nsh"
Name "AI Reaction Studio 0.4 Preview"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\AI Reaction Studio"
RequestExecutionLevel user
SetCompressor /SOLID zlib
VIProductVersion "0.4.0.0"
VIAddVersionKey /LANG=1033 "ProductName" "AI Reaction Studio"
VIAddVersionKey /LANG=1033 "FileDescription" "AI Reaction Studio development preview installer"
VIAddVersionKey /LANG=1033 "FileVersion" "0.4.0.0"
VIAddVersionKey /LANG=1033 "LegalCopyright" "AI Reaction Studio contributors"
!define MUI_ICON "..\..\apps\reaction-studio\assets\icon.ico"
!define MUI_UNICON "..\..\apps\reaction-studio\assets\icon.ico"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\..\LICENSE-MIT"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Function .onInit
  check_running:
    FindWindow $0 "" "AI Reaction Studio"
    StrCmp $0 0 app_closed
    IfSilent app_running_error
    MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "Close AI Reaction Studio before updating, then choose Retry." IDRETRY check_running
    app_running_error:
      SetErrorLevel 2
      Abort
  app_closed:
  SetShellVarContext current
  ${IfNot} ${RunningX64}
    MessageBox MB_OK|MB_ICONEXCLAMATION "This package requires 64-bit Windows."
    Abort
  ${EndIf}
  SetRegView 64
  ${DisableX64FSRedirection}
  IfFileExists "$WINDIR\System32\vcruntime140.dll" 0 missing
  IfFileExists "$WINDIR\System32\vcruntime140_1.dll" 0 missing
  IfFileExists "$WINDIR\System32\msvcp140.dll" ready missing
  missing:
    ${EnableX64FSRedirection}
    MessageBox MB_OK|MB_ICONEXCLAMATION "The local llama.cpp runtime requires Microsoft Visual C++ x64 runtime (vcruntime140.dll, vcruntime140_1.dll and msvcp140.dll), which was not detected. Install that prerequisite and run this installer again. No system components have been changed."
    Abort
  ready:
    ${EnableX64FSRedirection}
FunctionEnd

Section "AI Reaction Studio" Main
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  ClearErrors
  File /r "${STAGE}\*.*"
  IfErrors 0 files_ready
    SetErrorLevel 3
    Abort "Could not install all files. Close running copies of the app and retry."
  files_ready:
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateDirectory "$SMPROGRAMS\AI Reaction Studio"
  CreateShortcut "$SMPROGRAMS\AI Reaction Studio\AI Reaction Studio.lnk" "$INSTDIR\ai-reaction-studio.exe" "" "$INSTDIR\ai-reaction-studio.exe"
  CreateShortcut "$DESKTOP\AI Reaction Studio.lnk" "$INSTDIR\ai-reaction-studio.exe" "" "$INSTDIR\ai-reaction-studio.exe"
  CreateDirectory "$LOCALAPPDATA\AI Reaction Studio\Projects"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\AIReactionStudio" "DisplayName" "AI Reaction Studio 0.4 Preview"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\AIReactionStudio" "DisplayVersion" "0.4.0"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\AIReactionStudio" "Publisher" "AI Reaction Studio contributors"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\AIReactionStudio" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\AIReactionStudio" "DisplayIcon" "$INSTDIR\ai-reaction-studio.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\AIReactionStudio" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\AIReactionStudio" "URLInfoAbout" "https://github.com/stuartraymor/AI-Reaction-Studio"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\AIReactionStudio" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\AIReactionStudio" "NoRepair" 1
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  SetRegView 64
  !include "${UNINSTALL_MANIFEST}"
  Delete "$DESKTOP\AI Reaction Studio.lnk"
  Delete "$SMPROGRAMS\AI Reaction Studio\AI Reaction Studio.lnk"
  RMDir "$SMPROGRAMS\AI Reaction Studio"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\AIReactionStudio"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  ; The model cache, settings and projects are deliberately retained. No recursive deletion.
SectionEnd
