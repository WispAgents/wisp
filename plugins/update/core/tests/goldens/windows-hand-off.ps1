$ErrorActionPreference = 'Continue'
Start-Sleep -Seconds 1
Stop-Process -Id 4242 -Force -ErrorAction SilentlyContinue
Wait-Process -Id 4242 -Timeout 30 -ErrorAction SilentlyContinue
if (Get-Process -Id 4242 -ErrorAction SilentlyContinue) { Set-Content -LiteralPath 'C:\Users\Ann O''Brien\.wisp\plugins\wisp.update\update-exit.txt' -Value 'busy'; exit 1 }
Copy-Item -LiteralPath 'C:\Users\Ann O''Brien\AppData\Roaming\npm\node_modules\wisp-agents\node_modules\@wisp\core-win32-x64\bin\wisp.exe' -Destination 'C:\Users\Ann O''Brien\.wisp\plugins\wisp.update\wisp-old.exe' -Force -ErrorAction SilentlyContinue
try { $p = Start-Process -FilePath cmd.exe -ArgumentList '/d /s /c "npm install -g wisp-agents@latest > "C:\Users\Ann O''Brien\.wisp\plugins\wisp.update\update-npm.log" 2>&1"' -Wait -PassThru -WindowStyle Hidden -ErrorAction Stop; $code = $p.ExitCode } catch { $code = 'start' }
Set-Content -LiteralPath 'C:\Users\Ann O''Brien\.wisp\plugins\wisp.update\update-exit.txt' -Value $code
if (Test-Path -LiteralPath 'C:\Users\Ann O''Brien\AppData\Roaming\npm\node_modules\wisp-agents\node_modules\@wisp\core-win32-x64\bin\wisp.exe') { Remove-Item -LiteralPath 'C:\Users\Ann O''Brien\.wisp\plugins\wisp.update\wisp-old.exe' -Force -ErrorAction SilentlyContinue; Start-Process -FilePath cmd.exe -ArgumentList '/S /C ""node" "C:\\Users\\Ann O''Brien\\AppData\\Roaming\\npm\\node_modules\\wisp-agents\\bin\\wisp.js" --browser --no-open --port 4477"' -WindowStyle Hidden } else { Start-Process -FilePath 'C:\Users\Ann O''Brien\.wisp\plugins\wisp.update\wisp-old.exe' -ArgumentList '--browser --no-open --port 4477' -WindowStyle Hidden }
