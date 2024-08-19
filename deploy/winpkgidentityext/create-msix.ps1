Remove-Item .\juliaup.msix
& 'C:\Program Files (x86)\Windows Kits\10\bin\10.0.22000.0\x64\makeappx.exe' pack /d .\msix\ /p .\juliaup.msix /nv

$params = @{
        Endpoint = "https://eus.codesigning.azure.net/"
        CodeSigningAccountName = "juliahubwincertsaccount"
        CertificateProfileName = "JuliaHubWinCert"
        FilesFolder = "."
        FilesFolderFilter = "msix"
        FileDigest = "SHA256"
        TimestampRfc3161 = "http://timestamp.acs.microsoft.com"
        TimestampDigest = "SHA256"
        ExcludeManagedIdentityCredential = $True
        ExcludeEnvironmentCredential = $True
        ExcludeWorkloadIdentityCredential = $True
        ExcludeSharedTokenCacheCredential = $True
        ExcludeVisualStudioCredential = $True
        ExcludeVisualStudioCodeCredential = $True
        ExcludeAzurePowerShellCredential = $True
        ExcludeAzureDeveloperCliCredential = $True
        ExcludeInteractiveBrowserCredential = $True
    }

Invoke-TrustedSigning @params

Move-Item .\juliaup.msix ..\..\target\debug\ -Force
