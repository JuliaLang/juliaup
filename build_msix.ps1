mkdir -Force output\main
Remove-Item -Recurse output\main\*

$versions = Get-Content versions.json | ConvertFrom-Json
[version]$version = $versions.JuliaAppPackage.Version

push-location msix
&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\MakeAppx.exe" build /f PackagingLayout.xml /op ..\output\main /pv $version /bv $version
pop-location

Copy-Item msix\Julia.appinstaller output\main
Copy-Item msix\index.html output\main
