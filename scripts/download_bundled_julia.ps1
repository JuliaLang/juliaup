$x64Versions = Get-Content $PSScriptRoot\..\versiondb\versiondb-x86_64-pc-windows-msvc.json | ConvertFrom-Json
$x64VersionFromChannel = $x64Versions.AvailableChannels.release.Version
$x64DownloadUrl = $x64Versions.AvailableVersions.$x64VersionFromChannel.UrlPath
$x64Filename = Split-Path $x64DownloadUrl -leaf

$x86Versions = Get-Content $PSScriptRoot\..\versiondb\versiondb-i686-pc-windows-msvc.json | ConvertFrom-Json
$x86VersionFromChannel = $x86Versions.AvailableChannels.release.Version
$x86DownloadUrl = $x86Versions.AvailableVersions.$x86VersionFromChannel.UrlPath
$x86Filename = Split-Path $x86DownloadUrl -leaf

mkdir -Force $PSScriptRoot\..\target\bundledjulia\downloads
mkdir -Force $PSScriptRoot\..\target\bundledjulia\extracted

if (-Not (Test-Path $PSScriptRoot\..\"target\bundledjulia\downloads\$x64Filename"))
{
    Invoke-WebRequest "https://julialang-s3.julialang.org/$x64DownloadUrl" -OutFile $PSScriptRoot\..\"target\bundledjulia\downloads\$x64Filename"
    mkdir -Force $PSScriptRoot\..\target\bundledjulia\extracted\x64
    Remove-Item $PSScriptRoot\..\target\bundledjulia\extracted\x64\* -Force -Recurse    
    tar -xvzf $PSScriptRoot\..\"target\bundledjulia\downloads\$x64Filename" -C $PSScriptRoot\..\target\bundledjulia\extracted\x64 --strip-components=1
}

if (-Not (Test-Path $PSScriptRoot\..\"target\bundledjulia\downloads\$x86Filename"))
{
    Invoke-WebRequest "https://julialang-s3.julialang.org/$x86DownloadUrl" -OutFile $PSScriptRoot\..\"target\bundledjulia\downloads\$x86Filename"
    mkdir -Force $PSScriptRoot\..\target\bundledjulia\extracted\x86
    Remove-Item $PSScriptRoot\..\target\bundledjulia\extracted\x86\* -Force -Recurse
    tar -xvzf $PSScriptRoot\..\"target\bundledjulia\downloads\$x86Filename" -C $PSScriptRoot\..\target\bundledjulia\extracted\x86 --strip-components=1
}
