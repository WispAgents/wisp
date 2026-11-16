$ErrorActionPreference = 'Continue'
$trace = 'C:\Users\Ann O''Brien\.wisp\plugins\wisp.update\update-trace.txt'
function Step($m) { Add-Content -LiteralPath $trace -Value ('{0:HH:mm:ss.fff} {1}' -f (Get-Date), $m) -ErrorAction SilentlyContinue }
Set-Content -LiteralPath $trace -Value 'waiter for 4242' -ErrorAction SilentlyContinue
Start-Sleep -Seconds 1
$w = Get-Process -Id 4242 -ErrorAction SilentlyContinue
if ($w) { try { $null = $w.Handle } catch { Step "no handle: $_" }; Stop-Process -InputObject $w -Force -ErrorAction SilentlyContinue; try { $gone = $w.WaitForExit(30000) } catch { $gone = -not (Get-Process -Id 4242 -ErrorAction SilentlyContinue) }; if (-not $gone) { Step 'wisp did not stop'; Set-Content -LiteralPath 'C:\Users\Ann O''Brien\.wisp\plugins\wisp.update\update-exit.txt' -Value 'busy'; exit 1 } }
Step 'wisp is gone'
Copy-Item -LiteralPath 'C:\Users\Ann O''Brien\AppData\Roaming\npm\node_modules\wisp-agents\node_modules\wisp-agents-win32-x64\bin\wisp.exe' -Destination 'C:\Users\Ann O''Brien\.wisp\plugins\wisp.update\wisp-old.exe' -Force -ErrorAction SilentlyContinue
Step ("npm on PATH: " + ((& where.exe npm 2>$null) -join '; '))
try { $si = New-Object System.Diagnostics.ProcessStartInfo -ArgumentList 'cmd.exe'; $si.Arguments = '/d /s /c "npm install -g wisp-agents@latest > "C:\Users\Ann O''Brien\.wisp\plugins\wisp.update\update-npm.log" 2>&1"'; $si.UseShellExecute = $false; $si.CreateNoWindow = $true; $p = [System.Diagnostics.Process]::Start($si); $p.WaitForExit(); $code = $p.ExitCode } catch { $code = 'start'; Step "cmd.exe did not start: $_" }
Step "npm exited: $code"
Set-Content -LiteralPath 'C:\Users\Ann O''Brien\.wisp\plugins\wisp.update\update-exit.txt' -Value $code
if (Test-Path -LiteralPath 'C:\Users\Ann O''Brien\AppData\Roaming\npm\node_modules\wisp-agents\node_modules\wisp-agents-win32-x64\bin\wisp.exe') { Remove-Item -LiteralPath 'C:\Users\Ann O''Brien\.wisp\plugins\wisp.update\wisp-old.exe' -Force -ErrorAction SilentlyContinue; Step 'restart'; Start-Process -FilePath cmd.exe -ArgumentList '/S /C ""node" "C:\\Users\\Ann O''Brien\\AppData\\Roaming\\npm\\node_modules\\wisp-agents\\bin\\wisp.js" --browser --no-open --port 4477"' -WindowStyle Hidden } else { Step 'restart the copy set aside'; Start-Process -FilePath 'C:\Users\Ann O''Brien\.wisp\plugins\wisp.update\wisp-old.exe' -ArgumentList '--browser --no-open --port 4477' -WindowStyle Hidden }
Step 'done'
