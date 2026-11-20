<#
.SYNOPSIS
    Registers or unregisters Windows file associations for Trazo in HKCU
    (no administrator privileges required).
.PARAMETER ExePath
    Path to trazo.exe. Defaults to .\trazo.exe next to this script or in the
    current working directory.
.PARAMETER Unregister
    Removes the per-user Trazo file association registry entries.
#>
[CmdletBinding()]
param(
    [string]$ExePath = "",
    [switch]$Unregister
)

$ErrorActionPreference = "Stop"
$progId = "Trazo.Project"
$classesRoot = "HKCU:\Software\Classes"

function Notify-ShellAssociationChange {
    Add-Type -Namespace Win32 -Name ShellNotify -MemberDefinition @"
[System.Runtime.InteropServices.DllImport("shell32.dll")]
public static extern void SHChangeNotify(uint wEventId, uint uFlags, System.IntPtr dwItem1, System.IntPtr dwItem2);
"@ -ErrorAction SilentlyContinue
    [Win32.ShellNotify]::SHChangeNotify(0x08000000, 0x0000, [IntPtr]::Zero, [IntPtr]::Zero)
}

if ($Unregister) {
    Remove-Item -Path "$classesRoot\$progId" -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item -Path "$classesRoot\Applications\trazo.exe" -Recurse -Force -ErrorAction SilentlyContinue
    Remove-ItemProperty -Path "$classesRoot\.trazo\OpenWithProgids" -Name $progId -ErrorAction SilentlyContinue
    Remove-ItemProperty -Path "$classesRoot\.trazo" -Name "(Default)" -ErrorAction SilentlyContinue
    Remove-ItemProperty -Path "$classesRoot\.json\OpenWithProgids" -Name $progId -ErrorAction SilentlyContinue
    Notify-ShellAssociationChange
    Write-Host "Associação de arquivos do Trazo removida para o usuário atual."
    exit 0
}

if ([string]::IsNullOrWhiteSpace($ExePath)) {
    $candidates = @(
        (Join-Path $PSScriptRoot "trazo.exe"),
        (Join-Path (Get-Location) "trazo.exe"),
        (Join-Path $PSScriptRoot "..\target\release\trazo.exe")
    )
    $ExePath = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
}

if ([string]::IsNullOrWhiteSpace($ExePath) -or -not (Test-Path $ExePath)) {
    throw "trazo.exe não encontrado. Informe o caminho com -ExePath 'C:\caminho\trazo.exe'."
}

$resolvedExe = (Resolve-Path $ExePath).Path
$command = "`"$resolvedExe`" `"%1`""

New-Item -Path "$classesRoot\$progId\DefaultIcon" -Force | Out-Null
New-Item -Path "$classesRoot\$progId\shell\open\command" -Force | Out-Null
Set-ItemProperty -Path "$classesRoot\$progId" -Name "(Default)" -Value "Projeto Trazo"
Set-ItemProperty -Path "$classesRoot\$progId\DefaultIcon" -Name "(Default)" -Value "`"$resolvedExe`",0"
Set-ItemProperty -Path "$classesRoot\$progId\shell\open\command" -Name "(Default)" -Value $command

New-Item -Path "$classesRoot\Applications\trazo.exe\SupportedTypes" -Force | Out-Null
New-Item -Path "$classesRoot\Applications\trazo.exe\shell\open\command" -Force | Out-Null
Set-ItemProperty -Path "$classesRoot\Applications\trazo.exe" -Name "FriendlyAppName" -Value "Trazo"
Set-ItemProperty -Path "$classesRoot\Applications\trazo.exe\SupportedTypes" -Name ".json" -Value ""
Set-ItemProperty -Path "$classesRoot\Applications\trazo.exe\SupportedTypes" -Name ".trazo" -Value ""
Set-ItemProperty -Path "$classesRoot\Applications\trazo.exe\shell\open\command" -Name "(Default)" -Value $command

New-Item -Path "$classesRoot\.trazo\OpenWithProgids" -Force | Out-Null
Set-ItemProperty -Path "$classesRoot\.trazo" -Name "(Default)" -Value $progId
Set-ItemProperty -Path "$classesRoot\.trazo\OpenWithProgids" -Name $progId -Value ""

New-Item -Path "$classesRoot\.json\OpenWithProgids" -Force | Out-Null
Set-ItemProperty -Path "$classesRoot\.json\OpenWithProgids" -Name $progId -Value ""

Notify-ShellAssociationChange
Write-Host "Trazo associado com sucesso em $resolvedExe."
