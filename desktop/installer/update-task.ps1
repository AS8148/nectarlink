# SPDX-License-Identifier: GPL-3.0-or-later
# Registers the "Nectarlink Update" task (run by the installer, as an
# administrator). The task runs the app as SYSTEM with --update-service,
# which installs the latest GitHub release when it's newer, after checking
# it against the release's SHA-256 sums (desktop/app/src/updater.rs). It
# takes no input from whoever starts it. Signed-in users may start it, so
# updates need no administrator prompt; only SYSTEM and administrators may
# change or delete it.
param(
    [Parameter(Mandatory)][string]$Exe
)
$ErrorActionPreference = 'Stop'
$name = 'Nectarlink Update'

$action = New-ScheduledTaskAction -Execute $Exe -Argument '--update-service'
$principal = New-ScheduledTaskPrincipal -UserId 'S-1-5-18' -LogonType ServiceAccount -RunLevel Highest
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries `
    -MultipleInstances IgnoreNew -ExecutionTimeLimit (New-TimeSpan -Minutes 30)
Register-ScheduledTask -TaskName $name -Action $action -Principal $principal -Settings $settings `
    -Description 'Installs new versions of NectarLink from its GitHub releases.' -Force | Out-Null

# SYSTEM and administrators: full control. Authenticated users: read and run.
$service = New-Object -ComObject Schedule.Service
$service.Connect()
$service.GetFolder('\').GetTask($name).SetSecurityDescriptor('D:(A;;FA;;;SY)(A;;FA;;;BA)(A;;GRGX;;;AU)', 0)
