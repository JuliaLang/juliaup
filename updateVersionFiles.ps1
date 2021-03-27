$versionString = Get-Content VERSION

[version]$version = $versionString

$cVersionHeader = @"
#define JULIA_APP_VERSION_MAJOR $($version.major)
#define JULIA_APP_VERSION_MINOR $($version.minor)
#define JULIA_APP_VERSION_REVISION $($version.build)
#define JULIA_APP_VERSION_BUILD $($version.revision)
"@

$cVersionHeader | Out-File  -FilePath juliaup/version.h
$cVersionHeader | Out-File  -FilePath launcher/version.h

[xml]$xmlDoc = Get-Content msix\Julia.appinstaller
$xmlDoc.AppInstaller.Version = $version
$xmlDoc.AppInstaller.MainBundle.Version = $version
$xmlDoc.AppInstaller.MainBundle.Uri = "https://winjulia.s3-us-west-1.amazonaws.com/Julia-$version/Julia.appxbundle"
$xmlDoc.Save("msix\Julia.appinstaller")
