param(
    [string]$Source,
    [string]$OutputDirectory,
    [string]$DataDirectory,
    [string]$Script,
    [string]$Voice = 'kokoro-heart'
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
if (-not $OutputDirectory) {
    $OutputDirectory = Join-Path $env:LOCALAPPDATA ('AI Reaction Studio\Projects\Prototype-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
}
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path (Join-Path $OutputDirectory 'reaction.fcproj')) { throw 'Choose a new output directory; a project already exists here.' }
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
if (-not $DataDirectory) { $DataDirectory = Join-Path $env:LOCALAPPDATA 'AI Reaction Studio\Data' }
$env:FILMCRAFT_DATA_DIR = [IO.Path]::GetFullPath($DataDirectory)
$env:REACTION_VOICE = $Voice
Push-Location $repo
try {
    & cargo build --locked -p filmcraft-cli --example reaction_prototype --bin filmcraft-cli --features filmcraft-engine/neural-voices
    if ($LASTEXITCODE) { throw 'Build failed.' }
    $cli = Join-Path $repo 'target\debug\filmcraft-cli.exe'
    & $cli --data-dir $env:FILMCRAFT_DATA_DIR exec tts.downloadVoices wait=true
    if ($LASTEXITCODE) { throw 'Voice download failed.' }
    if (-not $Source) {
        $Source = Join-Path $OutputDirectory 'source-fixture.mp4'
        $settings = @{path=$Source; format='h264'; width=640; height=360; fps=24; range='custom'; startSeconds=0; endSeconds=6; wait=$true; hardwareEncoding='off'} | ConvertTo-Json -Compress
        & $cli --demo exec file.exportMedia $settings
        if ($LASTEXITCODE) { throw 'Fixture export failed.' }
    }
    $prototypeArgs = @([IO.Path]::GetFullPath($Source), $OutputDirectory)
    if ($Script) { $prototypeArgs += [IO.Path]::GetFullPath($Script) }
    & (Join-Path $repo 'target\debug\examples\reaction_prototype.exe') @prototypeArgs
    if ($LASTEXITCODE) { throw 'Reaction generation failed; inspect the error above. Existing project/assets are preserved.' }
    Write-Output ('Created ' + (Join-Path $OutputDirectory 'reaction.mp4'))
    Write-Output 'Open reaction.fcproj in FilmCraft to edit the separate video, narration and graphic tracks.'
}
finally { Pop-Location }
