if ( (git status --porcelain | Measure-Object -Line ).Lines -ne 0) 
{
    Write-Output "Cannot run this script with git changes pending."
    exit
}

$versions = Get-Content versions.json | ConvertFrom-Json

$oldAppVersion = [version]$versions.JuliaAppPackage.Version
$newAppVersion = [version]::new($oldAppVersion.Major, $oldAppVersion.Minor, $oldAppVersion.Build+1, $oldAppVersion.Revision)
$versions.JuliaAppPackage.Version = $newAppVersion.ToString()

$versions | ConvertTo-Json | Out-File versions.json

$bundledJuliaVersion = $versions.JuliaAppPackage.BundledJuliaVersion

$cVersionHeader = @"
#define JULIA_APP_VERSION_MAJOR $(([version]$versions.JuliaAppPackage.Version).major)
#define JULIA_APP_VERSION_MINOR $(([version]$versions.JuliaAppPackage.Version).minor)
#define JULIA_APP_VERSION_REVISION $(([version]$versions.JuliaAppPackage.Version).build)
#define JULIA_APP_VERSION_BUILD $(([version]$versions.JuliaAppPackage.Version).revision)
#define JULIA_APP_BUNDLED_JULIA "$($versions.JuliaAppPackage.BundledJuliaSemVersion)"
"@

$cVersionHeader | Out-File  -FilePath launcher/version.h



# TODO Bundle x86 binaries from Juliaup once we have them
$packageLayout = [xml]@"
<PackagingLayout xmlns="http://schemas.microsoft.com/appx/makeappx/2017">
  <PackageFamily ID="Julia-$($versions.JuliaAppPackage.Version)" FlatBundle="false" ManifestPath="appxmanifest.xml" ResourceManager="false">
    <Package ID="Julia-x64-$($versions.JuliaAppPackage.Version)" ProcessorArchitecture="x64">
      <Files>
        <File DestinationPath="Julia\*" SourcePath="..\build\output\x64\Release\launcher\*" />
        <File DestinationPath="Juliaup\**" SourcePath="..\build\juliaup\x64\**" />
        <File DestinationPath="Images\*.png" SourcePath="Images\*.png" />
        <File DestinationPath="Public\Fragments\*" SourcePath="Fragments\*" />
        <File DestinationPath="Public\Fragments\julia.ico" SourcePath="..\launcher\julia.ico" />
        <File DestinationPath="BundledJulia\**" SourcePath="..\build\juliaversions\x64\julia-$bundledJuliaVersion\**" />
        <File DestinationPath="VersionsDB\juliaup-versionsdb-winnt-x64.json" SourcePath="..\build\versiondb\juliaup-versionsdb-winnt-x64.json" />
      </Files>
    </Package>
    <Package ID="Julia-x86-$($versions.JuliaAppPackage.Version)" ProcessorArchitecture="x86">
      <Files>
        <File DestinationPath="Julia\*" SourcePath="..\build\output\Win32\Release\launcher\*" />
        <File DestinationPath="Juliaup\**" SourcePath="..\build\juliaup\x86\**" />
        <File DestinationPath="Images\*.png" SourcePath="Images\*.png" />
        <File DestinationPath="Public\Fragments\*" SourcePath="Fragments\*" />
        <File DestinationPath="Public\Fragments\julia.ico" SourcePath="..\launcher\julia.ico" />
        <File DestinationPath="BundledJulia\**" SourcePath="..\build\juliaversions\x86\julia-$bundledJuliaVersion\**" />
        <File DestinationPath="VersionsDB\juliaup-versionsdb-winnt-x86.json" SourcePath="..\build\versiondb\juliaup-versionsdb-winnt-x86.json" />
      </Files>
    </Package>
    <Package ID="Julia-ARM64-$($versions.JuliaAppPackage.Version)" ProcessorArchitecture="ARM64">
      <Files>
        <File DestinationPath="Julia\*" SourcePath="..\build\output\Win32\Release\launcher\*" />
        <File DestinationPath="Juliaup\**" SourcePath="..\build\juliaup\ARM64\**" />
        <File DestinationPath="Images\*.png" SourcePath="Images\*.png" />
        <File DestinationPath="Public\Fragments\*" SourcePath="Fragments\*" />
        <File DestinationPath="Public\Fragments\julia.ico" SourcePath="..\launcher\julia.ico" />
        <File DestinationPath="BundledJulia\**" SourcePath="..\build\juliaversions\x64\julia-$bundledJuliaVersion\**" />
        <File DestinationPath="VersionsDB\juliaup-versionsdb-winnt-arm64.json" SourcePath="..\build\versiondb\juliaup-versionsdb-winnt-arm64.json" />
      </Files>
    </Package>
  </PackageFamily>
</PackagingLayout>
"@
$packageLayout.Save("msix\PackagingLayout.xml")

$juliaVersionsJuliaFile = @"
JULIA_APP_VERSION = v"$($newAppVersion.Major).$($newAppVersion.Minor).$($newAppVersion.Build)"
"@

$juliaVersionsJuliaFile | Out-File .\Juliaup\src\versions_database.jl

git add .
git commit -m "Update version to v$($newAppVersion.ToString())"
git tag "v$($newAppVersion.ToString())"
