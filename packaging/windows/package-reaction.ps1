param(
    [Parameter(Mandatory=$true)][string]$NsisDirectory,
    [Parameter(Mandatory=$true)][string]$RuntimeDirectory,
    [Parameter(Mandatory=$true)][string]$EditorDirectory,
    [string]$OutputDirectory,
    [switch]$SkipBuild
)
$ErrorActionPreference='Stop'
$repo=(Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if(-not $OutputDirectory){$OutputDirectory=Join-Path $repo 'dist\reaction'}
$OutputDirectory=[IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
Push-Location $repo
try {
    if(-not $SkipBuild){
        $env:CARGO_PROFILE_DEV_DEBUG='0';$env:CARGO_INCREMENTAL='0'
        & cargo build --locked -p filmcraft-reaction-studio --bins
        if($LASTEXITCODE){throw 'Reaction Studio build failed'}
    }
    $stage=Join-Path $repo ('target\reaction-package\stage-'+(Get-Date -Format 'yyyyMMdd-HHmmss'))
    New-Item -ItemType Directory -Force -Path $stage,(Join-Path $stage 'runtime'),(Join-Path $stage 'editor') | Out-Null
    foreach($bin in @('ai-reaction-studio.exe','reaction-pipeline.exe')){Copy-Item -LiteralPath (Join-Path $repo ('target\debug\'+$bin)) -Destination $stage}
    foreach($bin in @('filmcraft.exe','filmcraft-cli.exe')){Copy-Item -LiteralPath (Join-Path $EditorDirectory $bin) -Destination (Join-Path $stage 'editor')}
    Copy-Item -LiteralPath (Join-Path $RuntimeDirectory 'llama-server.exe') -Destination (Join-Path $stage 'runtime')
    Get-ChildItem -LiteralPath $RuntimeDirectory -Filter '*.dll' | Copy-Item -Destination (Join-Path $stage 'runtime')
    Get-ChildItem -LiteralPath $RuntimeDirectory -Filter 'LICENSE*' | Copy-Item -Destination (Join-Path $stage 'runtime')
    foreach($file in @('LICENSE-MIT','LICENSE-APACHE','REACTION-STUDIO.md','REACTION-STUDIO-DEVELOPMENT.md')){Copy-Item -LiteralPath (Join-Path $repo $file) -Destination $stage}
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'REACTION-THIRD-PARTY.txt') -Destination $stage
    Copy-Item -LiteralPath (Join-Path $repo 'apps\reaction-studio\models.json') -Destination $stage
    Copy-Item -LiteralPath (Join-Path $repo 'target\reaction-third-party-licenses.txt') -Destination (Join-Path $stage 'THIRD-PARTY-LICENSES.txt')
    $manifest=Join-Path $stage 'uninstall-files.nsh'
    $lines=@()
    foreach($file in Get-ChildItem -LiteralPath $stage -Recurse -File){
        $relative=[IO.Path]::GetRelativePath($stage,$file.FullName)
        if($relative -notmatch '^[a-zA-Z0-9_.\-\\]+$'){throw "Unsupported package filename: $relative"}
        $lines+='Delete "$INSTDIR\'+$relative+'"'
    }
    $lines+='RMDir "$INSTDIR\runtime"';$lines+='RMDir "$INSTDIR\editor"'
    # Keep the generated uninstall manifest outside the packaged folder.
    $manifest=Join-Path (Split-Path $stage -Parent) ((Split-Path $stage -Leaf)+'-uninstall.nsh')
    Set-Content -LiteralPath $manifest -Value $lines -Encoding utf8
    $installer=Join-Path $OutputDirectory 'AI-Reaction-Studio-0.2.0-Preview-Setup.exe'
    & (Join-Path $NsisDirectory 'makensis.exe') ('/DSTAGE='+$stage) ('/DOUTPUT='+$installer) ('/DUNINSTALL_MANIFEST='+$manifest) (Join-Path $PSScriptRoot 'reaction-studio.nsi')
    if($LASTEXITCODE){throw 'Installer compilation failed'}
    Get-FileHash -LiteralPath $installer -Algorithm SHA256 | Format-List
    Write-Output $installer
}
finally{Pop-Location}
