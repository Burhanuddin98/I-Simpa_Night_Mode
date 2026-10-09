; I-Simpa Night Mode, the Windows installer (rebuild plan M13): one NSIS build, per user, no admin.
; Built by build-installer.ps1, which stages the payload with tools\devtools\payload.ps1 (the same
; files the portable package ships) and writes PAYLOAD_NSH, the explicit list of every file: the
; uninstaller deletes exactly that list, so a project or a run a user saved inside the install folder
; is never touched, and the folder stays only while it holds one.
;
; Defines (all from build-installer.ps1): VERSION (Cargo's workspace version, what About shows),
; COMMIT, OUTFILE, PAYLOAD_NSH, LICENSE_FILE, ICON, WEBVIEW2 (Microsoft's bootstrapper), SIZE_KB.
;
; Command line, beside NSIS's own /S (silent) and /D=<folder> (last, unquoted):
;   /MENUNAME=<name>  the Start menu entry's name (default "I-Simpa Night Mode"); the M13 bed
;                     installs under a scratch name so nothing of a real install is overwritten.

Unicode true
ManifestDPIAware true
SetCompressor /SOLID lzma
RequestExecutionLevel user

!include MUI2.nsh
!include FileFunc.nsh
!include LogicLib.nsh
!include x64.nsh

!define PRODUCT "I-Simpa Night Mode"
!define PUBLISHER "Dockyard"
; Parity A42: the per-user ProgID that opens a .simpa in the app.
!define PROGID "DockyardNightMode.simpa"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\I-Simpa Night Mode"
!define CLASSES "Software\Classes"
!define FILEEXTS "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.simpa"
; The WebView2 Runtime's client id (Microsoft's distribution guide).
!define WV2_GUID "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"

Name "${PRODUCT}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\${PRODUCT}"
InstallDirRegKey HKCU "${UNINST_KEY}" "InstallLocation"
BrandingText "${PRODUCT} ${VERSION} (${COMMIT})"
ShowInstDetails show
ShowUninstDetails show

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${PRODUCT}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "FileDescription" "${PRODUCT} ${VERSION} setup"
VIAddVersionKey "CompanyName" "${PUBLISHER}"
VIAddVersionKey "LegalCopyright" "GPL-3.0"
VIAddVersionKey "Comments" "commit ${COMMIT}"

!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"
!define MUI_ABORTWARNING
; The licence page informs; the GPL asks for no acceptance to use the program.
!define MUI_LICENSEPAGE_BUTTON "$(^NextBtn)"
!define MUI_LICENSEPAGE_TEXT_BOTTOM "Night Mode is free software under the GPL-3.0. Its mesher, TetGen, is under the AGPL-3.0 (TETGEN-LICENSE.txt); THIRD-PARTY-NOTICES.txt lists the rest. Both are installed beside the app."
!insertmacro MUI_PAGE_LICENSE "${LICENSE_FILE}"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\app.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Start ${PRODUCT}"
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

; PAYLOAD_INSTALL and PAYLOAD_UNINSTALL, one File and one Delete line per staged file.
!include "${PAYLOAD_NSH}"

Var MenuName

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "${PRODUCT} needs 64-bit Windows 10 or 11." /SD IDOK
    Abort
  ${EndIf}
  SetShellVarContext current
  SetRegView 64
  StrCpy $MenuName "${PRODUCT}"
  ${GetParameters} $0
  ClearErrors
  ${GetOptions} $0 "/MENUNAME=" $1
  ${IfNot} ${Errors}
  ${AndIf} $1 != ""
    StrCpy $MenuName $1
  ${EndIf}
FunctionEnd

; The WebView2 Runtime: present (per machine or per user), or installed by Microsoft's bootstrapper,
; carried in the installer and run from its temporary folder. Unelevated, it installs per user.
Function EnsureWebView2
  ReadRegStr $0 HKLM "SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\${WV2_GUID}" "pv"
  ${If} $0 == ""
  ${OrIf} $0 == "0.0.0.0"
    ReadRegStr $0 HKCU "Software\Microsoft\EdgeUpdate\Clients\${WV2_GUID}" "pv"
  ${EndIf}
  ${If} $0 != ""
  ${AndIf} $0 != "0.0.0.0"
    DetailPrint "Microsoft Edge WebView2 Runtime $0 is installed."
    Return
  ${EndIf}
  DetailPrint "Installing the Microsoft Edge WebView2 Runtime (the app's window needs it)..."
  InitPluginsDir
  File "/oname=$PLUGINSDIR\MicrosoftEdgeWebview2Setup.exe" "${WEBVIEW2}"
  ExecWait '"$PLUGINSDIR\MicrosoftEdgeWebview2Setup.exe" /silent /install' $1
  DetailPrint "The WebView2 bootstrapper answered $1."
  ReadRegStr $0 HKCU "Software\Microsoft\EdgeUpdate\Clients\${WV2_GUID}" "pv"
  ${If} $0 == ""
    ReadRegStr $0 HKLM "SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\${WV2_GUID}" "pv"
  ${EndIf}
  ${If} $0 == ""
  ${OrIf} $0 == "0.0.0.0"
    MessageBox MB_ICONEXCLAMATION "The WebView2 Runtime could not be installed (code $1). ${PRODUCT} is installed but its window will not open until the runtime is: https://developer.microsoft.com/microsoft-edge/webview2/" /SD IDOK
  ${EndIf}
FunctionEnd

Section "Install"
  SetShellVarContext current
  SetRegView 64
  Call EnsureWebView2

  ; Over an earlier install: its own uninstaller removes its own file list first, in place.
  ${If} ${FileExists} "$INSTDIR\uninstall.exe"
    DetailPrint "Removing the version installed here before..."
    ExecWait '"$INSTDIR\uninstall.exe" /S _?=$INSTDIR'
    Delete "$INSTDIR\uninstall.exe"
  ${EndIf}

  !insertmacro PAYLOAD_INSTALL
  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  CreateShortcut "$SMPROGRAMS\$MenuName.lnk" "$INSTDIR\app.exe" "" "$INSTDIR\app.exe" 0 SW_SHOWNORMAL "" "Room acoustics: SPPS and TCR behind a new interface"

  ; Parity A42: a .simpa opens in the app, per user. A default another program held is kept to give back.
  ReadRegStr $0 HKCU "${CLASSES}\.simpa" ""
  ${If} $0 != ""
  ${AndIf} $0 != "${PROGID}"
    WriteRegStr HKCU "${UNINST_KEY}" "PreviousSimpaProgId" "$0"
  ${EndIf}
  WriteRegStr HKCU "${CLASSES}\.simpa" "" "${PROGID}"
  WriteRegStr HKCU "${CLASSES}\.simpa\OpenWithProgids" "${PROGID}" ""
  WriteRegStr HKCU "${CLASSES}\${PROGID}" "" "${PRODUCT} project"
  WriteRegStr HKCU "${CLASSES}\${PROGID}\DefaultIcon" "" '"$INSTDIR\app.exe",0'
  WriteRegStr HKCU "${CLASSES}\${PROGID}\shell\open\command" "" '"$INSTDIR\app.exe" --project "%1"'
  ; SHCNE_ASSOCCHANGED: Explorer rereads the association now, not at the next sign-in.
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'

  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "${PRODUCT}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKCU "${UNINST_KEY}" "Comments" "commit ${COMMIT}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" '"$INSTDIR\app.exe",0'
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegStr HKCU "${UNINST_KEY}" "StartMenuShortcut" "$SMPROGRAMS\$MenuName.lnk"
  WriteRegStr HKCU "${UNINST_KEY}" "URLInfoAbout" "https://github.com/Burhanuddin98/I-Simpa_Night_Mode"
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "EstimatedSize" ${SIZE_KB}
SectionEnd

Function un.onInit
  SetShellVarContext current
  SetRegView 64
FunctionEnd

Section "Uninstall"
  ; app.exe cannot be deleted while it runs: ask, rather than leave half an install.
  retry:
  ClearErrors
  Delete "$INSTDIR\app.exe"
  ${If} ${Errors}
    MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "${PRODUCT} is running from $INSTDIR. Close it, then press Retry." /SD IDCANCEL IDRETRY retry
    Abort "${PRODUCT} is still running; nothing was removed."
  ${EndIf}

  !insertmacro PAYLOAD_UNINSTALL
  Delete "$INSTDIR\uninstall.exe"
  ; Not /r: a folder holding a user's own file (a project, its runs) stays, with only that file in it.
  RMDir "$INSTDIR"

  ReadRegStr $0 HKCU "${UNINST_KEY}" "StartMenuShortcut"
  ${If} $0 != ""
    Delete "$0"
  ${EndIf}

  ; A42 undone: the extension goes back to what it was, our ProgID goes.
  ReadRegStr $0 HKCU "${CLASSES}\.simpa" ""
  ${If} $0 == "${PROGID}"
    ReadRegStr $1 HKCU "${UNINST_KEY}" "PreviousSimpaProgId"
    ${If} $1 != ""
      WriteRegStr HKCU "${CLASSES}\.simpa" "" "$1"
    ${Else}
      DeleteRegValue HKCU "${CLASSES}\.simpa" ""
    ${EndIf}
  ${EndIf}
  DeleteRegValue HKCU "${CLASSES}\.simpa\OpenWithProgids" "${PROGID}"
  DeleteRegKey /ifempty HKCU "${CLASSES}\.simpa\OpenWithProgids"
  DeleteRegKey /ifempty HKCU "${CLASSES}\.simpa"
  DeleteRegKey HKCU "${CLASSES}\${PROGID}"
  ; Explorer's own record of what opened a .simpa.
  DeleteRegValue HKCU "${FILEEXTS}\OpenWithProgids" "${PROGID}"
  DeleteRegKey /ifempty HKCU "${FILEEXTS}\OpenWithProgids"
  DeleteRegKey /ifempty HKCU "${FILEEXTS}"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'

  DeleteRegKey HKCU "${UNINST_KEY}"
SectionEnd
