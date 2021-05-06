mkdir -Force output\main
Remove-Item -Recurse output\main\*

$versions = Get-Content versions.json | ConvertFrom-Json
[version]$version = $versions.JuliaAppPackage.Version

push-location msix
&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\MakeAppx.exe" build /f PackagingLayout.xml /op ..\output\main /pv $version /bv $version
pop-location

&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\signtool" sign /fd SHA256 /sha1 467A053795F772FC96BC766AD85D2C039E4DF9B3 output\main\*
Copy-Item msix\Julia.appinstaller output\main
Copy-Item msix\index.html output\main
