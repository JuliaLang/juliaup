Copy-Item .\deploy\msi\License.rtf .\target\release
wix build -ext WixToolset.UI.wixext .\deploy\msi\Julia.wxs -b .\target\release\ -arch x86 -o target\msi\Julia.msi
