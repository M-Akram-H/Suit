; Suit's setup program for Windows, made with NSIS (https://nsis.sourceforge.io):
;
;   makensis -DVERSION=0.1.0 -DBIN=<folder with suit.exe> -DROOT=<the source> -DOUT=<file> suit.nsi
;
; It installs for the person who runs it, into their own folders, and so needs no
; administrator. build-windows.sh runs it.

Unicode True
!include "MUI2.nsh"

!define NAME "Suit"
!define PUBLISHER "Mohamed Akram Hlali"
!define WEBSITE "https://github.com/M-Akram-H/Suit"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${NAME}"

Name "${NAME}"
OutFile "${OUT}"
InstallDir "$LOCALAPPDATA\Programs\${NAME}"
InstallDirRegKey HKCU "Software\${NAME}" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${NAME}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "FileDescription" "${NAME} setup"
VIAddVersionKey "CompanyName" "${PUBLISHER}"
VIAddVersionKey "LegalCopyright" "Copyright (c) 2026 ${PUBLISHER}"

!define MUI_ICON "${ROOT}/packaging/windows/suit.ico"
!define MUI_UNICON "${ROOT}/packaging/windows/suit.ico"
!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TEXT "Suit is a visual editor for Manim slides: animated mathematical presentations, made by clicking instead of writing Python.$\r$\n$\r$\nSuit draws with Manim, which is installed separately, for your user account, right after this setup. That needs Python 3.12 and an internet connection.$\r$\n$\r$\nClick Next to continue."
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_TEXT "Set up Manim now (needed once, takes a few minutes)"
!define MUI_FINISHPAGE_RUN_FUNCTION SetUpManim
!define MUI_FINISHPAGE_LINK "Suit's manual"
!define MUI_FINISHPAGE_LINK_LOCATION "${WEBSITE}/blob/main/docs/README.md"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${ROOT}/LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

; PowerShell is asked to run this one script whatever the computer's rule about scripts
; is; the rule itself is left as it was.
!define RUN_SETUP '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\setup-manim.ps1"'

Function SetUpManim
    Exec '${RUN_SETUP}'
FunctionEnd

Section "Suit" SecSuit
    SectionIn RO
    SetOutPath "$INSTDIR"
    File "${BIN}/suit.exe"
    File "${BIN}/suit-cli.exe"
    File "${ROOT}/packaging/windows/suit.ico"
    File "${ROOT}/packaging/windows/setup-manim.ps1"
    File "${ROOT}/requirements.txt"
    File "/oname=constraints.txt" "${ROOT}/docker/constraints.txt"
    File "/oname=LICENSE.txt" "${ROOT}/LICENSE"
    SetOutPath "$INSTDIR\examples"
    File "${ROOT}/examples/*.suit.json"
    SetOutPath "$INSTDIR"

    WriteUninstaller "$INSTDIR\uninstall.exe"
    WriteRegStr HKCU "Software\${NAME}" "InstallDir" "$INSTDIR"

    CreateDirectory "$SMPROGRAMS\${NAME}"
    CreateShortcut "$SMPROGRAMS\${NAME}\${NAME}.lnk" "$INSTDIR\suit.exe" "" "$INSTDIR\suit.ico"
    CreateShortcut "$SMPROGRAMS\${NAME}\Set up Manim for Suit.lnk" \
        "$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" \
        '-NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\setup-manim.ps1"' "$INSTDIR\suit.ico"
    CreateShortcut "$SMPROGRAMS\${NAME}\Example decks.lnk" "$INSTDIR\examples"
    CreateShortcut "$SMPROGRAMS\${NAME}\Uninstall Suit.lnk" "$INSTDIR\uninstall.exe"
    CreateShortcut "$DESKTOP\${NAME}.lnk" "$INSTDIR\suit.exe" "" "$INSTDIR\suit.ico"

    ; So that Windows lists it among the installed programs.
    WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${NAME}"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "${PUBLISHER}"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "URLInfoAbout" "${WEBSITE}"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\suit.ico"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
    WriteRegStr HKCU "${UNINSTALL_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
    WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
    WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
    WriteRegDWORD HKCU "${UNINSTALL_KEY}" "EstimatedSize" 30000
SectionEnd

Section "Uninstall"
    Delete "$INSTDIR\suit.exe"
    Delete "$INSTDIR\suit-cli.exe"
    Delete "$INSTDIR\suit.ico"
    Delete "$INSTDIR\setup-manim.ps1"
    Delete "$INSTDIR\requirements.txt"
    Delete "$INSTDIR\constraints.txt"
    Delete "$INSTDIR\LICENSE.txt"
    Delete "$INSTDIR\examples\*.suit.json"
    RMDir "$INSTDIR\examples"
    Delete "$INSTDIR\uninstall.exe"
    RMDir "$INSTDIR"

    Delete "$SMPROGRAMS\${NAME}\*.lnk"
    RMDir "$SMPROGRAMS\${NAME}"
    Delete "$DESKTOP\${NAME}.lnk"

    DeleteRegKey HKCU "${UNINSTALL_KEY}"
    DeleteRegKey HKCU "Software\${NAME}"

    ; Manim, and what it has typeset, are the person's own and several hundred megabytes:
    ; they go only if that is wanted. A silent uninstall leaves them.
    IfSilent done
    IfFileExists "$LOCALAPPDATA\${NAME}\*.*" 0 done
    MessageBox MB_YESNO|MB_ICONQUESTION "Also remove the Manim that was set up for Suit, and its cache?$\r$\n$\r$\n$LOCALAPPDATA\${NAME}" IDNO done
    RMDir /r "$LOCALAPPDATA\${NAME}"
    done:
SectionEnd
