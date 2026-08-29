' Runs t3here.ps1 with no console window. Used by the context-menu entry only;
' from a terminal use t3here.cmd instead, so you can see the output.
Dim fso, shell, folder, target
Set fso   = CreateObject("Scripting.FileSystemObject")
Set shell = CreateObject("WScript.Shell")

folder = fso.GetParentFolderName(WScript.ScriptFullName)

If WScript.Arguments.Count > 0 Then target = WScript.Arguments(0) Else target = folder
' A trailing backslash would escape the closing quote below.
If Right(target, 1) = "\" Then target = Left(target, Len(target) - 1)

shell.Run "powershell -NoProfile -ExecutionPolicy Bypass -File """ & folder & _
          "\t3here.ps1"" """ & target & """", 0, False
