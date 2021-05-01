$versions = Get-Content versions.json | ConvertFrom-Json

$cVersionHeader = @"
#define JULIA_APP_VERSION_MAJOR $(([version]$versions.JuliaAppPackage.Version).major)
#define JULIA_APP_VERSION_MINOR $(([version]$versions.JuliaAppPackage.Version).minor)
#define JULIA_APP_VERSION_REVISION $(([version]$versions.JuliaAppPackage.Version).build)
#define JULIA_APP_VERSION_BUILD $(([version]$versions.JuliaAppPackage.Version).revision)
"@

$cVersionHeader | Out-File  -FilePath juliaup/version.h
$cVersionHeader | Out-File  -FilePath launcher/version.h

$appInstaller = [xml]@"
<?xml version="1.0" encoding="utf-8"?>
<AppInstaller xmlns="http://schemas.microsoft.com/appx/appinstaller/2017/2" Version="$($versions.JuliaAppPackage.Version)" Uri="https://winjulia.s3-us-west-1.amazonaws.com/Julia.appinstaller">
  <MainBundle Name="JuliaApp" Publisher="CN=David Anthoff, O=David Anthoff, S=California, C=US" Version="$($versions.JuliaAppPackage.Version)" Uri="https://winjulia.s3-us-west-1.amazonaws.com/JuliaApp-$($versions.JuliaAppPackage.Version).appxbundle" />
  <OptionalPackages>
    $($versions.OptionalJuliaPackages | ? {$_.IncludeByDefault -eq $TRUE} | % {
        $juliaVersion = [version]$_.JuliaVersion
        '<Bundle Name="Julia-{0}" Publisher="CN=David Anthoff, O=David Anthoff, S=California, C=US" Version="{1}" Uri="https://winjulia.s3-us-west-1.amazonaws.com/Julia-{0}-{1}.appxbundle" />                 
        ' -f "$($juliaVersion.major).$($juliaVersion.minor).$($juliaVersion.build)", $_.Version
    })
  </OptionalPackages>
  <RelatedPackages> 
    $($versions.OptionalJuliaPackages | ? {$_.IncludeByDefault -eq $TRUE} | % {
        $juliaVersion = [version]$_.JuliaVersion
        '
        <Bundle Name="Julia-x86-{0}" Publisher="CN=David Anthoff, O=David Anthoff, S=California, C=US" Version="{1}" Uri="https://winjulia.s3-us-west-1.amazonaws.com/Julia-x86-{0}-{1}.appxbundle" />
        ' -f "$($juliaVersion.major).$($juliaVersion.minor).$($juliaVersion.build)", $_.Version
    })   
    $($versions.OptionalJuliaPackages | ? {$_.IncludeByDefault -eq $FALSE} | % {
        $juliaVersion = [version]$_.JuliaVersion
        '<Bundle Name="Julia-{0}" Publisher="CN=David Anthoff, O=David Anthoff, S=California, C=US" Version="{1}" Uri="https://winjulia.s3-us-west-1.amazonaws.com/Julia-{0}-{1}.appxbundle" />
        <Bundle Name="Julia-x86-{0}" Publisher="CN=David Anthoff, O=David Anthoff, S=California, C=US" Version="{1}" Uri="https://winjulia.s3-us-west-1.amazonaws.com/Julia-x86-{0}-{1}.appxbundle" />
        ' -f "$($juliaVersion.major).$($juliaVersion.minor).$($juliaVersion.build)", $_.Version
    })
  </RelatedPackages>
  <Dependencies>
    <Package Name="Microsoft.VCLibs.140.00.UWPDesktop" Publisher="CN=Microsoft Corporation, O=Microsoft Corporation, L=Redmond, S=Washington, C=US" Version="14.0.29231.0" ProcessorArchitecture="x64" Uri="https://winjulia.s3-us-west-1.amazonaws.com/Microsoft.VCLibs.x64.14.00.Desktop.appx" />
    <Package Name="Microsoft.VCLibs.140.00.UWPDesktop" Publisher="CN=Microsoft Corporation, O=Microsoft Corporation, L=Redmond, S=Washington, C=US" Version="14.0.29231.0" ProcessorArchitecture="x86" Uri="https://winjulia.s3-us-west-1.amazonaws.com/Microsoft.VCLibs.x86.14.00.Desktop.appx" />
    <Package Name="Microsoft.VCLibs.140.00" Publisher="CN=Microsoft Corporation, O=Microsoft Corporation, L=Redmond, S=Washington, C=US" Version="14.0.29231.0" ProcessorArchitecture="x64" Uri="https://winjulia.s3-us-west-1.amazonaws.com/Microsoft.VCLibs.x64.14.00.appx" />
    <Package Name="Microsoft.VCLibs.140.00" Publisher="CN=Microsoft Corporation, O=Microsoft Corporation, L=Redmond, S=Washington, C=US" Version="14.0.29231.0" ProcessorArchitecture="x86" Uri="https://winjulia.s3-us-west-1.amazonaws.com/Microsoft.VCLibs.x86.14.00.appx" />
  </Dependencies>
  <UpdateSettings>
    <OnLaunch HoursBetweenUpdateChecks="0" />
  </UpdateSettings>
</AppInstaller>
"@
$appInstaller.Save("msix\Julia.appinstaller")

$packageLayout = [xml]@"
<PackagingLayout xmlns="http://schemas.microsoft.com/appx/makeappx/2017">
  <PackageFamily ID="JuliaApp-$($versions.JuliaAppPackage.Version)" FlatBundle="true" ManifestPath="appxmanifest.xml" ResourceManager="false">
    <Package ID="JuliaApp-x64-$($versions.JuliaAppPackage.Version)" ProcessorArchitecture="x64">
      <Files>
        <File DestinationPath="Julia\julia.exe" SourcePath="..\build\output\x64\Release\launcher\julia.exe" />
        <File DestinationPath="Julia\juliaup.exe" SourcePath="..\build\output\x64\Release\juliaup\juliaup.exe" />
        <File DestinationPath="Images\*.png" SourcePath="Images\*.png" />
        <File DestinationPath="Public\Fragments\Julia.json" SourcePath="Fragments\Julia.json" />
      </Files>
    </Package>
    <Package ID="JuliaApp-x86-$($versions.JuliaAppPackage.Version)" ProcessorArchitecture="x86">
      <Files>
        <File DestinationPath="Julia\julia.exe" SourcePath="..\build\output\Win32\Release\launcher\julia.exe" />
        <File DestinationPath="Julia\juliaup.exe" SourcePath="..\build\output\Win32\Release\juliaup\juliaup.exe" />
        <File DestinationPath="Images\*.png" SourcePath="Images\*.png" />
        <File DestinationPath="Public\Fragments\Julia.json" SourcePath="Fragments\Julia.json" />
      </Files>
    </Package>   
  </PackageFamily>
  $($versions.OptionalJuliaPackages | % {
    $juliaVersion = [version]$_.JuliaVersion
    '
    <PrebuiltPackage Path="..\output\optional\Julia-{0}-{1}.appxbundle" />
    <PrebuiltPackage Path="..\output\optional\Julia-x86-{0}-{1}.appxbundle" />
    ' -f "$($juliaVersion.major).$($juliaVersion.minor).$($juliaVersion.build)", $_.Version
  })
</PackagingLayout>
"@
$packageLayout.Save("msix\PackagingLayout.xml")

$packageLayoutOptionalPackages = [xml]@"
<PackagingLayout xmlns="http://schemas.microsoft.com/appx/makeappx/2017">
    $($versions.OptionalJuliaPackages | % {
        $juliaVersion = [version]$_.JuliaVersion
        '<PackageFamily ID="Julia-{0}-{1}" Optional="true" ManifestPath="julia-{0}-appxmanifest.xml" ResourceManager="false">
            <Package ID="Julia-{0}-x64-{1}" ProcessorArchitecture="x64">
                <Files>
                    <File DestinationPath="Julia\**" SourcePath="..\optionalpackages\win64\julia-{0}\**" />
                    <File DestinationPath="Images\*.png" SourcePath="Images\*.png" />
                </Files>
            </Package>
            <Package ID="Julia-{0}-x86-{1}" ProcessorArchitecture="x86">
                <Files>
                    <File DestinationPath="Julia\**" SourcePath="..\optionalpackages\win32\julia-{0}\**" />
                    <File DestinationPath="Images\*.png" SourcePath="Images\*.png" />
                </Files>
            </Package>            
        </PackageFamily>
        <PackageFamily ID="Julia-x86-{0}-{1}" Optional="true" ManifestPath="julia-x86-{0}-appxmanifest.xml" ResourceManager="false">
            <Package ID="Julia-x86-{0}-x64-{1}" ProcessorArchitecture="x64">
                <Files>
                    <File DestinationPath="Julia\**" SourcePath="..\optionalpackages\win32\julia-{0}\**" />
                    <File DestinationPath="Images\*.png" SourcePath="Images\*.png" />
                </Files>
            </Package>
            <Package ID="Julia-x86-{0}-x86-{1}" ProcessorArchitecture="x86">
                <Files>
                    <File DestinationPath="Julia\**" SourcePath="..\optionalpackages\win32\julia-{0}\**" />
                    <File DestinationPath="Images\*.png" SourcePath="Images\*.png" />
                </Files>
            </Package>            
        </PackageFamily>
        ' -f "$($juliaVersion.major).$($juliaVersion.minor).$($juliaVersion.build)", $_.Version
    })
</PackagingLayout>
"@
$packageLayoutOptionalPackages.Save("msix\PackagingLayoutOptionalPackages.xml")

$versions.OptionalJuliaPackages | ForEach-Object -Parallel {
  [version]$juliaVersion = $_.JuliaVersion
  $shortJuliaVersion = "$($juliaVersion.major).$($juliaVersion.minor).$($juliaVersion.build)"
  $packageversion = $_.Version

  $appmanifest = [xml]@"
<?xml version="1.0" encoding="utf-8"?>
  <Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10" 
    xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10" 
    xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
    xmlns:desktop="http://schemas.microsoft.com/appx/manifest/desktop/windows10"
    xmlns:uap3="http://schemas.microsoft.com/appx/manifest/uap/windows10/3" IgnorableNamespaces="uap3">
    <Identity Name="Julia-$shortJuliaVersion" Version="$packageversion" Publisher="CN=David Anthoff, O=David Anthoff, S=California, C=US" ProcessorArchitecture="neutral" />
    <Properties>
        <DisplayName>Julia $shortJuliaVersion</DisplayName>
        <PublisherDisplayName>David Anthoff</PublisherDisplayName>
        <Description>Julia is a high-level, high-performance, dynamic programming language</Description>
        <Logo>Images/StoreLogo.png</Logo>
    </Properties>
    <Resources>
        <Resource Language="en-us" />
    </Resources>
    <Dependencies>
        <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.15063.0" MaxVersionTested="10.0.15063.0" />
        <uap3:MainPackageDependency Name="JuliaApp"/>
    </Dependencies>
  </Package>
"@
  $appmanifest.Save("msix/julia-$shortJuliaVersion-appxmanifest.xml")

  $appmanifestx86 = [xml]@"
<?xml version="1.0" encoding="utf-8"?>
    <Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10" 
      xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10" 
      xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
      xmlns:desktop="http://schemas.microsoft.com/appx/manifest/desktop/windows10"
      xmlns:uap3="http://schemas.microsoft.com/appx/manifest/uap/windows10/3" IgnorableNamespaces="uap3">
      <Identity Name="Julia-x86-$shortJuliaVersion" Version="$packageversion" Publisher="CN=David Anthoff, O=David Anthoff, S=California, C=US" ProcessorArchitecture="neutral" />
      <Properties>
          <DisplayName>Julia $shortJuliaVersion (32 bit)</DisplayName>
          <PublisherDisplayName>David Anthoff</PublisherDisplayName>
          <Description>Julia is a high-level, high-performance, dynamic programming language</Description>
          <Logo>Images/StoreLogo.png</Logo>
      </Properties>
      <Resources>
          <Resource Language="en-us" />
      </Resources>
      <Dependencies>
          <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.15063.0" MaxVersionTested="10.0.15063.0" />
          <uap3:MainPackageDependency Name="JuliaApp"/>
      </Dependencies>
    </Package>
"@
    $appmanifestx86.Save("msix/julia-x86-$shortJuliaVersion-appxmanifest.xml")  
}

$juliaVersionsCppFile = @"
#include "pch.h"

std::vector<JuliaVersion> JuliaVersionsDatabase::getJuliaVersions() {
	std::vector<JuliaVersion> juliaVersions = { 
    $($versions.OptionalJuliaPackages | % {
      $parts = $_.JuliaVersion.Split('.')
      "JuliaVersion{$($parts[0]), $($parts[1]), $($parts[2])}"
    } | Join-String -Separator ', ')
	};
	return juliaVersions;
}
"@
$juliaVersionsCppFile | Out-File .\juliaup\generatedjuliaversions.cpp
$juliaVersionsCppFile | Out-File .\launcher\generatedjuliaversions.cpp
