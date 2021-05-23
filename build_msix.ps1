mkdir -Force .\output
mkdir -Force .\build\msix
Remove-Item .\build\msix\*

$versions = Get-Content versions.json | ConvertFrom-Json
[version]$version = $versions.JuliaAppPackage.Version

push-location msix
&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\MakeAppx.exe" build /f PackagingLayout.xml /op ..\build\msix /pv $version /bv $version
pop-location

# &"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\signtool" sign /fd SHA256 /sha1 E70A5E7F058A0E4FCAAC9CC604C44EC8588D1C59 build\msix\*

Move-Item .\build\msix\*.appxbundle .\output -Force
