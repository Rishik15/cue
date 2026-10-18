; Purpose: extra uninstall steps for the NSIS installer.
; Contents: NSIS_HOOK_POSTUNINSTALL removes the install-path key the installer writes under the publisher, which is not cleaned up otherwise.
!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegKey SHCTX "Software\Rishik\Cue"
!macroend
