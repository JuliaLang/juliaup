ri output\main\*

push-location msix
&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.18362.0\x64\MakeAppx.exe" build /f PackagingLayout.xml /op ..\output\main /pv 1.0.0.3 /bv 1.0.0.3
pop-location

&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.18362.0\x64\signtool" sign /fd SHA256 /sha1 66EED318F62344B3A1F148660EAA97C108DDFFF4 output\main\*
cpi msix\Julia.appinstaller output\main
cpi msix\index.html output\main
