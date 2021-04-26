mkdir -Force output\main
Remove-Item -Recurse output\main\*

[version]$version = Get-Content VERSION

push-location msix
&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\MakeAppx.exe" build /f PackagingLayout.xml /op ..\output\main /pv $version /bv $version
pop-location

&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\signtool" sign /fd SHA256 /sha1 E70A5E7F058A0E4FCAAC9CC604C44EC8588D1C59 output\main\*
Copy-Item msix\Julia.appinstaller output\main
Copy-Item msix\index.html output\main
