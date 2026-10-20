; fw-ams: undo the system changes when the app is uninstalled (not on updates).
; Included by Tauri's NSIS template (tauri.conf.json: bundle > windows > nsis > installerHooks).
; PREUNINSTALL runs before Tauri checks for a running app and deletes the files.

!include LogicLib.nsh

!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    ; A running app would write its charge limit again, so close it first
    !insertmacro CheckIfAppIsRunning "$INSTDIR\${MAINBINARYNAME}.exe" "${PRODUCTNAME}"

    ; Charge limit back to the EC's own value, charge current limit removed (no admin rights)
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --restore-system'

    ; Helper service: resets the tuner, restores the Windows power plan, deletes its profiles
    ; and log. Needs admin rights, hence one UAC prompt - only when it is installed.
    nsExec::Exec 'sc.exe query fw-ams-tuner'
    Pop $0
    ${If} $0 == "0"
    ${AndIf} ${FileExists} "$INSTDIR\fw-ams-service.exe"
      ClearErrors
      ExecShellWait "runas" "$INSTDIR\fw-ams-service.exe" "uninstall --purge" SW_HIDE
      ${If} ${Errors}
        MessageBox MB_ICONEXCLAMATION|MB_OK "Der fw-ams-Hilfsdienst wurde nicht entfernt (Admin-Abfrage abgelehnt?).$\r$\n$\r$\nZum Entfernen fw-ams erneut installieren und unter Einstellungen → Werkseinstellungen den Hilfsdienst mit entfernen." /SD IDOK
      ${EndIf}
    ${EndIf}

    ; Autostart entry of tauri-plugin-autostart (current user, value named after the product)
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCTNAME}"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "${PRODUCTNAME}"
  ${EndIf}
!macroend
