; Purpose: extra uninstall steps for the NSIS installer.
; Contents: NSIS_HOOK_POSTUNINSTALL removes the install-path key the installer writes under the publisher and, unless this uninstall is part of an update, all of Cue's data, whatever the "Delete the application data" box said.
!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegKey SHCTX "Software\Rishik\Cue"
  ${If} $UpdateMode <> 1
    RmDir /r "$APPDATA\${BUNDLEID}"
    RmDir /r "$LOCALAPPDATA\${BUNDLEID}"
  ${EndIf}
!macroend
