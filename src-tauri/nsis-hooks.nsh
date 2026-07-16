; Custom NSIS install hooks for MD Preview.
;
; Tauri's built-in file-association macro (APP_ASSOCIATE) registers the ProgID
; and sets it as the class default for .md/.markdown, but it does NOT add the
; ProgID to each extension's `OpenWithProgids` key -- which is the canonical
; source Explorer reads to build its "Open with" list. These hooks add that,
; so "MD Preview" reliably appears under "Open with" even when it is not the
; default handler.

!macro NSIS_HOOK_POSTINSTALL
  ; List MD Preview in Explorer's "Open with" menu for both extensions.
  WriteRegStr SHCTX "Software\Classes\.md\OpenWithProgids" "Markdown Document" ""
  WriteRegStr SHCTX "Software\Classes\.markdown\OpenWithProgids" "Markdown Document" ""
  ; Friendly names shown in the chooser / Explorer "Type" column.
  WriteRegStr SHCTX "Software\Classes\Markdown Document\Application" "ApplicationName" "MD Preview"
  WriteRegStr SHCTX "Software\Classes\Markdown Document" "FriendlyTypeName" "Markdown Document"
  ; Tell the shell associations changed so it refreshes without a reboot.
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, i 0, i 0)'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegValue SHCTX "Software\Classes\.md\OpenWithProgids" "Markdown Document"
  DeleteRegValue SHCTX "Software\Classes\.markdown\OpenWithProgids" "Markdown Document"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, i 0, i 0)'
!macroend
