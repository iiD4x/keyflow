# End-to-end test: types test triggers into a WinForms text box and verifies expansion via SendInput.
# Requires: KeyFlow running with snippets kftest1 -> keyflow-works and kftest2 -> A{tab}B{enter}C (mode instant).
$ErrorActionPreference = 'Continue'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$outFile = Join-Path $env:TEMP 'keyflow-e2e.txt'
if (Test-Path $outFile) { Remove-Item $outFile -Force }

$form = New-Object System.Windows.Forms.Form
$form.Text = 'KeyFlow E2E Target'
$form.Size = New-Object System.Drawing.Size(560, 220)
$form.StartPosition = 'CenterScreen'
$form.TopMost = $true

$box = New-Object System.Windows.Forms.TextBox
$box.Multiline = $true
$box.AcceptsTab = $true
$box.Dock = 'Fill'
$box.Font = New-Object System.Drawing.Font('Consolas', 14)
$form.Controls.Add($box)

$script:step = 0
$timer = New-Object System.Windows.Forms.Timer
$timer.Interval = 1400
$timer.Add_Tick({
  $script:step++
  $form.Activate()
  $form.BringToFront()
  Start-Sleep -Milliseconds 250
  if ($script:step -eq 1) {
    [System.Windows.Forms.SendKeys]::SendWait('kftest1')
  } elseif ($script:step -eq 2) {
    $box.Clear()
    [System.Windows.Forms.SendKeys]::SendWait('kftest2')
  } else {
    $timer.Stop()
    $box.Text | Out-File -FilePath $outFile -Encoding utf8
    $form.Close()
  }
})

$form.Add_Shown({
  $form.Activate()
  $form.BringToFront()
  $timer.Start()
})

[System.Windows.Forms.Application]::Run($form)
Write-Output 'E2E window closed'
if (Test-Path $outFile) {
  Write-Output 'RESULT-BEGIN'
  Get-Content -LiteralPath $outFile -Raw
  Write-Output 'RESULT-END'
}
