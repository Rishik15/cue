# File: scripts/lib/clipformats.ps1
# Purpose: tell whether the clipboard holds only plain text, which a harness can save and put back whole with GetText and SetText; anything else (an image, files, rich text) would be lost, so the harness refuses to start instead.
# Contents: Get-NonTextFormats (pure), Assert-TextClipboard.

# The clipboard formats that plain text does not account for. Input: the format names (GetDataObject().GetFormats()). Return: those names, empty when the clipboard holds only text or nothing.
function Get-NonTextFormats($formats) { @($formats | Where-Object { $_ -and $_ -notin "Text", "UnicodeText", "OEMText", "System.String", "Locale" }) }

# Throws, before anything is changed, when the clipboard holds more than plain text, naming what it holds and how to clear it. Needs System.Windows.Forms.
function Assert-TextClipboard {
  $data = [System.Windows.Forms.Clipboard]::GetDataObject()
  $other = Get-NonTextFormats $(if ($data) { $data.GetFormats() })
  if ($other.Count) { throw "The clipboard holds more than plain text ($($other -join ', ')), which this run could not put back. Copy some plain text or clear it (Win+V, Clear all) and run again." }
}
