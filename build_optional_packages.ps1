ri output\optional\*

&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.18362.0\x64\signtool" sign /fd SHA256 /sha1 66EED318F62344B3A1F148660EAA97C108DDFFF4 optionalpackages\julia-1.4.0\bin\*.exe
&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.18362.0\x64\signtool" sign /fd SHA256 /sha1 66EED318F62344B3A1F148660EAA97C108DDFFF4 optionalpackages\julia-1.4.0\bin\*.dll
&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.18362.0\x64\signtool" sign /fd SHA256 /sha1 66EED318F62344B3A1F148660EAA97C108DDFFF4 optionalpackages\julia-1.4.0\libexec\*.exe
&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.18362.0\x64\signtool" sign /fd SHA256 /sha1 66EED318F62344B3A1F148660EAA97C108DDFFF4 optionalpackages\julia-1.4.0\libexec\*.dll
&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.18362.0\x64\signtool" sign /fd SHA256 /sha1 66EED318F62344B3A1F148660EAA97C108DDFFF4 optionalpackages\julia-1.4.0\lib\julia\*.dll

&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.18362.0\x64\signtool" sign /fd SHA256 /sha1 66EED318F62344B3A1F148660EAA97C108DDFFF4 optionalpackages\julia-1.4.1\bin\*.exe
&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.18362.0\x64\signtool" sign /fd SHA256 /sha1 66EED318F62344B3A1F148660EAA97C108DDFFF4 optionalpackages\julia-1.4.1\bin\*.dll
&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.18362.0\x64\signtool" sign /fd SHA256 /sha1 66EED318F62344B3A1F148660EAA97C108DDFFF4 optionalpackages\julia-1.4.1\libexec\*.exe
&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.18362.0\x64\signtool" sign /fd SHA256 /sha1 66EED318F62344B3A1F148660EAA97C108DDFFF4 optionalpackages\julia-1.4.1\libexec\*.dll
&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.18362.0\x64\signtool" sign /fd SHA256 /sha1 66EED318F62344B3A1F148660EAA97C108DDFFF4 optionalpackages\julia-1.4.1\lib\julia\*.dll

&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.18362.0\x64\signtool" sign /fd SHA256 /sha1 66EED318F62344B3A1F148660EAA97C108DDFFF4 optionalpackages\julia-1.4.2\bin\*.exe
&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.18362.0\x64\signtool" sign /fd SHA256 /sha1 66EED318F62344B3A1F148660EAA97C108DDFFF4 optionalpackages\julia-1.4.2\bin\*.dll
&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.18362.0\x64\signtool" sign /fd SHA256 /sha1 66EED318F62344B3A1F148660EAA97C108DDFFF4 optionalpackages\julia-1.4.2\libexec\*.exe
&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.18362.0\x64\signtool" sign /fd SHA256 /sha1 66EED318F62344B3A1F148660EAA97C108DDFFF4 optionalpackages\julia-1.4.2\libexec\*.dll
&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.18362.0\x64\signtool" sign /fd SHA256 /sha1 66EED318F62344B3A1F148660EAA97C108DDFFF4 optionalpackages\julia-1.4.2\lib\julia\*.dll

push-location msix
&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.18362.0\x64\MakeAppx.exe" build /f PackagingLayoutOptionalPackages.xml /op ..\output\optional /pv 1.0.0.0 /bv 1.0.0.0
pop-location

&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.18362.0\x64\signtool" sign /fd SHA256 /sha1 66EED318F62344B3A1F148660EAA97C108DDFFF4 output\optional\*

copy-item "C:\Program Files (x86)\Microsoft SDKs\Windows Kits\10\ExtensionSDKs\Microsoft.VCLibs.Desktop\14.0\Appx\Retail\x64\Microsoft.VCLibs.x64.14.00.Desktop.appx" output\optional
copy-item "C:\Program Files (x86)\Microsoft SDKs\Windows Kits\10\ExtensionSDKs\Microsoft.VCLibs.Desktop\14.0\Appx\Retail\x86\Microsoft.VCLibs.x86.14.00.Desktop.appx" output\optional
