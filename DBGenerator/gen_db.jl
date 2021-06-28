using JSON, Query

function main()
    db_x64 = Dict{String,Any}("AvailableVersions"=>Dict{String,Any}(), "AvailableChannels"=>Dict{String,Any}())
    db_x86 = Dict{String,Any}("AvailableVersions"=>Dict{String,Any}(), "AvailableChannels"=>Dict{String,Any}())

    old_db = JSON.parsefile(joinpath(@__DIR__, "..", "versions.json"), use_mmap=false)

    versions = map(old_db["OptionalJuliaPackages"]) do i
        v = VersionNumber(i["JuliaVersion"])
        return (version=v, major="$(v.major)", minor="$(v.major).$(v.minor)")
    end

    minor_channels = versions |> @groupby(_.minor) |> @map({channel=key(_), version=first(_ |> @orderby_descending(i->i.version)).version})
    major_channels = versions |> @groupby(_.major) |> @map({channel=key(_), version=first(_ |> @orderby_descending(i->i.version)).version})
   
    for v in versions
        db_x64["AvailableVersions"]["$(v.version)~x64"] = Dict("Url"=>"https://julialang-s3.julialang.org/bin/winnt/x64/$(v.minor)/julia-$(v.version)-win64.tar.gz")
        db_x64["AvailableVersions"]["$(v.version)~x86"] = Dict("Url"=>"https://julialang-s3.julialang.org/bin/winnt/x86/$(v.minor)/julia-$(v.version)-win32.tar.gz")
        db_x86["AvailableVersions"]["$(v.version)~x86"] = Dict("Url"=>"https://julialang-s3.julialang.org/bin/winnt/x86/$(v.minor)/julia-$(v.version)-win32.tar.gz")

        db_x64["AvailableChannels"]["$(v.version)~x64"] = Dict("Version" => "$(v.version)~x64")
        db_x64["AvailableChannels"]["$(v.version)~x86"] = Dict("Version" => "$(v.version)~x86")
        db_x86["AvailableChannels"]["$(v.version)~x86"] = Dict("Version" => "$(v.version)~x86")
        db_x64["AvailableChannels"]["$(v.version)"] = Dict("Version" => "$(v.version)~x64")
        db_x86["AvailableChannels"]["$(v.version)"] = Dict("Version" => "$(v.version)~x86")
    end

    for c in minor_channels
        db_x64["AvailableChannels"]["$(c.channel)~x64"] = Dict("Version" => "$(c.version)~x64")
        db_x64["AvailableChannels"]["$(c.channel)~x86"] = Dict("Version" => "$(c.version)~x86")
        db_x86["AvailableChannels"]["$(c.channel)~x86"] = Dict("Version" => "$(c.version)~x86")
        db_x64["AvailableChannels"]["$(c.channel)"] = Dict("Version" => "$(c.version)~x64")
        db_x86["AvailableChannels"]["$(c.channel)"] = Dict("Version" => "$(c.version)~x86")
    end

    for c in major_channels
        db_x64["AvailableChannels"]["$(c.channel)~x64"] = Dict("Version" => "$(c.version)~x64")
        db_x64["AvailableChannels"]["$(c.channel)~x86"] = Dict("Version" => "$(c.version)~x86")
        db_x86["AvailableChannels"]["$(c.channel)~x86"] = Dict("Version" => "$(c.version)~x86")
        db_x64["AvailableChannels"]["$(c.channel)"] = Dict("Version" => "$(c.version)~x64")
        db_x86["AvailableChannels"]["$(c.channel)"] = Dict("Version" => "$(c.version)~x86")
    end

    db_x64["AvailableChannels"]["release~x64"] = Dict("Version" => "1.6.1~x64")
    db_x64["AvailableChannels"]["release~x86"] = Dict("Version" => "1.6.1~x86")
    db_x86["AvailableChannels"]["release~x86"] = Dict("Version" => "1.6.1~x86")
    db_x64["AvailableChannels"]["release"] = Dict("Version" => "1.6.1~x64")
    db_x86["AvailableChannels"]["release"] = Dict("Version" => "1.6.1~x86")

    db_x64["AvailableChannels"]["lts~x64"] = Dict("Version" => "1.0.5~x64")
    db_x64["AvailableChannels"]["lts~x86"] = Dict("Version" => "1.0.5~x86")
    db_x86["AvailableChannels"]["lts~x86"] = Dict("Version" => "1.0.5~x86")
    db_x64["AvailableChannels"]["lts"] = Dict("Version" => "1.0.5~x64")
    db_x86["AvailableChannels"]["lts"] = Dict("Version" => "1.0.5~x86")


    open(joinpath(@__DIR__, "..", "output", "juliaup-versionsdb-winnt-x64.json"), "w") do f
        JSON.print(f, db_x64, 4)
    end

    open(joinpath(@__DIR__, "..", "output", "juliaup-versionsdb-winnt-x86.json"), "w") do f
        JSON.print(f, db_x86, 4)
    end
end

main()
