$params = @{
        Endpoint = "https://eus.codesigning.azure.net/"
        CodeSigningAccountName = "juliahubwincertsaccount"
        CertificateProfileName = "JuliaHubWinCert"
        FilesFolder = "."
        FilesFolderFilter = "msix"
        FileDigest = "SHA256"
        TimestampRfc3161 = "http://timestamp.acs.microsoft.com"
        TimestampDigest = "SHA256"
    }

Invoke-TrustedSigning @params