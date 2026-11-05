!define MONITOR_PROTOCOL "Software\Classes\kvojps-monitor"
!define MONITOR_ACTIVATOR "{57165877-A42D-4FF3-A2D9-B8B94EF32F09}"
!define MONITOR_COM "Software\Classes\CLSID\${MONITOR_ACTIVATOR}"
!define MONITOR_IDENTITY "Software\Classes\AppUserModelId\${BUNDLEID}"
!define MONITOR_RUN "Software\Microsoft\Windows\CurrentVersion\Run"

Var OtherStartupCommand

!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    ; Tauri removes the Run value by product name. Preserve a portable copy's value.
    ReadRegStr $OtherStartupCommand HKCU "${MONITOR_RUN}" "${PRODUCTNAME}"
    ${If} $OtherStartupCommand == '$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --autostart'
      StrCpy $OtherStartupCommand ""
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    ${If} $OtherStartupCommand != ""
      WriteRegStr HKCU "${MONITOR_RUN}" "${PRODUCTNAME}" "$OtherStartupCommand"
    ${EndIf}

    ReadRegStr $0 HKCU "${MONITOR_PROTOCOL}" "MonitorOwner"
    ReadRegStr $1 HKCU "${MONITOR_PROTOCOL}\shell\open\command" ""
    ${If} $0 == "${BUNDLEID}"
    ${AndIf} $1 == '$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --notification-activate $\"%1$\"'
      DeleteRegKey HKCU "${MONITOR_PROTOCOL}"
    ${EndIf}

    ReadRegStr $0 HKCU "${MONITOR_COM}\LocalServer32" ""
    ${If} $0 == '$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --toast-activate'
      DeleteRegKey HKCU "${MONITOR_COM}\LocalServer32"
      DeleteRegKey /ifempty HKCU "${MONITOR_COM}"
      ReadRegStr $0 HKCU "${MONITOR_IDENTITY}" "CustomActivator"
      ${If} $0 == "${MONITOR_ACTIVATOR}"
        DeleteRegValue HKCU "${MONITOR_IDENTITY}" "CustomActivator"
        DeleteRegValue HKCU "${MONITOR_IDENTITY}" "DisplayName"
        DeleteRegKey /ifempty HKCU "${MONITOR_IDENTITY}"
      ${EndIf}
    ${EndIf}

    ReadRegStr $0 HKCU "${MANUPRODUCTKEY}" ""
    ${If} $0 == "$INSTDIR"
      DeleteRegValue HKCU "${MANUPRODUCTKEY}" ""
      DeleteRegValue HKCU "${MANUPRODUCTKEY}" "Installer Language"
      DeleteRegKey /ifempty HKCU "${MANUPRODUCTKEY}"
      DeleteRegKey /ifempty HKCU "${MANUKEY}"
    ${EndIf}
  ${EndIf}
!macroend
