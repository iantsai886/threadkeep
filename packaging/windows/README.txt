Threadkeep (early build, unsigned)

Install (no administrator rights needed):
  1. Unzip this folder.
  2. In PowerShell, inside the folder:  .\install.ps1
     (add -AddToPath to type "threadkeep" in any new terminal)
  3. Start the app:  threadkeep.exe app
     It opens in your browser and is reachable from this computer only.

Windows may warn that the program is from an unknown publisher: it is not signed yet.
Check the download against the .sha256 file that came with the zip before running it.

Connect an AI app from the app's setup screens, or from the command line:
  threadkeep.exe connect claude-desktop --space MyNotes:propose
  threadkeep.exe doctor

Uninstall:  .\uninstall.ps1   (removes the program and disconnects your AI apps; keeps your notes
and settings).

Your notes stay in your own folders as plain Markdown files. Settings are in %APPDATA%\Threadkeep.
