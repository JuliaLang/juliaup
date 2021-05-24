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

$cVersionHeader = @"
#define JULIA_APP_VERSION_MAJOR $(([version]$versions.JuliaAppPackage.Version).major)
#define JULIA_APP_VERSION_MINOR $(([version]$versions.JuliaAppPackage.Version).minor)
#define JULIA_APP_VERSION_REVISION $(([version]$versions.JuliaAppPackage.Version).build)
#define JULIA_APP_VERSION_BUILD $(([version]$versions.JuliaAppPackage.Version).revision)
"@

$cVersionHeader | Out-File  -FilePath launcher/version.h

$bundledJuliaVersion = $versions.JuliaAppPackage.BundledJuliaVersion

$packageLayout = [xml]@"
<PackagingLayout xmlns="http://schemas.microsoft.com/appx/makeappx/2017">
  <PackageFamily ID="Julia-$($versions.JuliaAppPackage.Version)" FlatBundle="false" ManifestPath="appxmanifest.xml" ResourceManager="false">
    <Package ID="Julia-x64-$($versions.JuliaAppPackage.Version)" ProcessorArchitecture="x64">
      <Files>
        <File DestinationPath="Julia\julia.exe" SourcePath="..\build\output\x64\Release\launcher\julia.exe" />
        <File DestinationPath="Juliaup\**" SourcePath="..\build\juliaup\x64\bin\**" />
        <File DestinationPath="Images\*.png" SourcePath="Images\*.png" />
        <File DestinationPath="Public\Fragments\Julia.json" SourcePath="Fragments\Julia.json" />
        <File DestinationPath="BundledJulia\**" SourcePath="..\optionalpackages\win64\julia-$bundledJuliaVersion\**" />
      </Files>
    </Package>
    <Package ID="Julia-x86-$($versions.JuliaAppPackage.Version)" ProcessorArchitecture="x86">
      <Files>
        <File DestinationPath="Julia\julia.exe" SourcePath="..\build\output\Win32\Release\launcher\julia.exe" />
        <File DestinationPath="Juliaup\**" SourcePath="..\build\juliaup\x86\bin\**" />
        <File DestinationPath="Images\*.png" SourcePath="Images\*.png" />
        <File DestinationPath="Public\Fragments\Julia.json" SourcePath="Fragments\Julia.json" />
        <File DestinationPath="BundledJulia\**" SourcePath="..\optionalpackages\win32\julia-$bundledJuliaVersion\**" />
      </Files>
    </Package>   
  </PackageFamily>
</PackagingLayout>
"@
$packageLayout.Save("msix\PackagingLayout.xml")

$juliaVersionsCppFile = @"
#include "pch.h"

std::vector<JuliaVersion> JuliaVersionsDatabase::getJuliaVersions() {
	std::vector<JuliaVersion> juliaVersions = { 
    $($versions.OptionalJuliaPackages | % {
      $parts = $_.JuliaVersion.Split('.')
      "JuliaVersion{$($parts[0]), $($parts[1]), $($parts[2])}"
    } | Join-String -Separator ', ')
	};
  std::sort(juliaVersions.begin(), juliaVersions.end(), [](const JuliaVersion& a, const JuliaVersion& b) {
		if (a.major == b.major) {
			if (a.minor == b.minor) {
				return a.patch < b.patch;
			}
			else {
				return a.minor < b.minor;
			}
		}
		else {
			return a.major < b.major;
		}
	});
	return juliaVersions;
}

std::wstring JuliaVersionsDatabase::getBundledJuliaVersion() {
  return std::wstring {L"$bundledJuliaVersion"};
}
"@
$juliaVersionsCppFile | Out-File .\launcher\generatedjuliaversions.cpp

$juliaVersionsJuliaFile = @"
JULIA_APP_VERSION = v"$($newAppVersion.Major).$($newAppVersion.Minor).$($newAppVersion.Build)"

JULIA_VERSIONS = [
  $($versions.OptionalJuliaPackages | % {
    $parts = $_.JuliaVersion.Split('.')
    "VersionNumber($($parts[0]), $($parts[1]), $($parts[2]))"
  } | Join-String -Separator ', ')
]
"@

$juliaVersionsJuliaFile | Out-File .\Juliaup\src\versions_database.jl

git add .
git commit -m "Update version to v$($newAppVersion.ToString())"
git tag "v$($newAppVersion.ToString())"
