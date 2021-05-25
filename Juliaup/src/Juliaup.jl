module Juliaup

import Downloads, Tar, CodecZlib, TOML

include("versions_database.jl")

function julia_main()
    try
        real_main()
    catch
        Base.invokelatest(Base.display_error, Base.catch_stack())
        return 1
    end
    return 0
end

function isValidJuliaVersion(version)
	return true
end

function isValidJuliaChannel(version)
	return true
end

function target_path_for_julia_version(platform, version)
	return joinpath(homedir(), ".julia", "juliaup", platform, "julia-$version")
end

function getJuliaVersionsThatMatchChannel(channelString)
	parts = split(channelString, '.')

	versionsThatWeCouldUse = VersionNumber[]

	# Collect all the known versions of Julia that exist that match our channel into a vector
	for currVersion in reverse(JULIA_VERSIONS)
		if length(parts) == 1 && parts[1] == string(currVersion.major)
			push!(versionsThatWeCouldUse, currVersion)
		elseif length(parts) == 2 && parts[1] == string(currVersion.major) && parts[2] == string(currVersion.minor)
			push!(versionsThatWeCouldUse, currVersion)
		end
	end

	return versionsThatWeCouldUse
end

function installJuliaVersion(platform::AbstractString, version::VersionNumber)
	secondary_platform_string = platform=="x64" ? "win64" : platform=="x86" ? "win32" : error("Unknown platform.")
	downloadUrl = "https://julialang-s3.julialang.org/bin/winnt/$platform/$(version.major).$(version.minor)/julia-$(version)-$secondary_platform_string.tar.gz"

	target_path = joinpath(homedir(), ".julia", "juliaup", platform)

	println("Installing Julia $version.")

	temp_file = Downloads.download(downloadUrl)

	try
		open(temp_file) do tar_gz
			tar = CodecZlib.GzipDecompressorStream(tar_gz)
			try
				mktempdir() do extract_temp_path
					Tar.extract(tar, extract_temp_path)
					mv(joinpath(extract_temp_path, "julia-$version"), joinpath(target_path, "julia-$version"), force=true)

					println("New version successfully installed.")
				end
			finally
				close(tar)
			end
		end

		
	finally
		rm(temp_file, force=true)
	end
end

function real_main()
    if length(ARGS)==0
        println("Julia Version Manager Preview")
		println()
		println("juliaup command line utility enables configuration of the default Julia version from the command line.")
		println()
		println("usage: juliaup [<command>] [<options>]")
		println()
		println("The following commands are available:")
		println()
		println("  setdefault    Set the default Julia version")
		println("  add           Add a specific Julia version to your system")
		println("  update        Update the current channel to the latest Julia version")
		println("  status        Show all installed Julia versions")
		println("  remove        Remove a Julia version from your system")
		println()
		println("For more details on a specific command, pass it the help argument. [-?] (not yet implemented)")
		println()
		println("The following options are available:")
		println("  -v,--version  Display the version of the tool")
		println("  --info        Display general info of the tool")
		println()
    elseif length(ARGS)>0

		if ARGS[1] == "-v" || ARGS[1] == "--version"
			if length(ARGS)==1
				println("v", JULIA_APP_VERSION)
			else
				println("ERROR: The ", ARGS[1], " argument does not accept any additional arguments.")
			end
		elseif ARGS[1] == "--info"
			if length(ARGS)==1
				println("Julia Version Manager Preview")
				println("Copyright (c) David Anthoff")
			else
				println("ERROR: The --info argument does not accept any additional arguments.")
			end
		elseif ARGS[1] == "setdefault"
			if length(ARGS)==2
				if isValidJuliaVersion(ARGS[2]) || isValidJuliaChannel(ARGS[2])
					juliaup_config_file_path = joinpath(homedir(), ".julia", "juliaup", "juliaup.toml")

					data = isfile(juliaup_config_file_path) ?
						TOML.parsefile(juliaup_config_file_path) :
						Dict()

					data["currentversion"] = ARGS[2]

					open(juliaup_config_file_path, "w") do f
						TOML.print(f, data)
					end

					println("Configured the default Julia version to be ", ARGS[2], ".")
				else
					# TODO Come up with a less hardcoded version of this.
					println("ERROR: '", ARGS[2], "' is not a valid Julia version. Valid values are '1.5.1', '1.5.2', '1.5.3', '1.5.4', '1.6.0' or '1.6.1'.")
				end
			else
				println("ERROR: The setdefault command only accepts one additional argument.")
			end
		elseif ARGS[1] == "add"
			if length(ARGS)==2
				if isValidJuliaVersion(ARGS[2])
					version_to_install = VersionNumber(ARGS[2])

					installJuliaVersion("x64", version_to_install)
				else
					# TODO Come up with a less hardcoded version of this.
					println("ERROR: '", ARGS[2], "' is not a valid Julia version. Valid values are '1.5.1', '1.5.2', '1.5.3', '1.5.4', '1.6.0' or '1.6.1'.")
				end
			else
				println("ERROR: The add command only accepts one additional argument.")
			end
		elseif ARGS[1] == "update" || ARGS[1] == "up"
			if length(ARGS)==1
				juliaVersionToUse = "1"

				julia_config_file_path = joinpath(homedir(), ".julia", "juliaup", "juliaup.toml")

				if isfile(julia_config_file_path)
					config_data = TOML.parsefile(julia_config_file_path)
					juliaVersionToUse = get(config_data, "currentversion", "1")
				end

				parts = split(juliaVersionToUse, '~')
				versionPart = parts[1]
				platformPart = length(parts) > 1 ? parts[2] : "x64";

				#  Now figure out whether we got a channel or a specific version.
				parts2 = split(versionPart, '.')

				if length(parts2) < 3
					publishedVersionsWeCouldUse = getJuliaVersionsThatMatchChannel(versionPart)

					if length(publishedVersionsWeCouldUse) > 0
							target_path = target_path_for_julia_version(platformPart, publishedVersionsWeCouldUse[1])

							if isdir(target_path)
								println("You already have the latest Julia version for the active channel installed.")
							else
								installJuliaVersion(platformPart, publishedVersionsWeCouldUse[1])
							end
					else
						println("You currently have a Julia channel configured for which no Julia versions exists. Nothing can be updated.")
					end
				else
				    println("You currently have a specific Julia version as your default configured. Only channel defaults can be updated.")
				end
			else
				println("ERROR: The update command does not accept any additional arguments.")
			end
		elseif ARGS[1] == "remove" || ARGS[1] == "rm"			

			if length(ARGS)==2

				if isValidJuliaVersion(ARGS[2])
					parts = split(ARGS[2], '~')

					juliaVersionToUninstall, juliaPlatform = if length(parts)==1
						parts[1], "x64"
					elseif length(parts)==2
						parts[1], parts[2]
					else
						error("Invalid version specifier.")
					end

					path_to_be_deleted = joinpath(homedir(), ".julia", "juliaup", juliaPlatform, "julia-$juliaVersionToUninstall")

					if isdir(path_to_be_deleted)
						rm(path_to_be_deleted, force=true, recursive=true)
						println("Julia $juliaVersionToUninstall successfully removed.")
					else
						println("Julia $juliaVersionToUninstall cannot be removed because it is currently not installed.")
					end
				else
					# TODO Come up with a less hardcoded version of this.
					println("ERROR: '", ARGS[2], "' is not a valid Julia version. Valid values are '1.5.1', '1.5.2', '1.5.3', '1.5.4', '1.6.0' or '1.6.1'.")
				end
			else
				println("ERROR: The remove command only accepts one additional argument.")
			end
		elseif ARGS[1] == "status" || ARGS[1] == "st"
			if length(ARGS)==1
				println("The following Julia versions are currently installed:")

				for platform in ["x64", "x86"]
					if isdir(joinpath(homedir(), ".julia", "juliaup", platform))
						for i in readdir(joinpath(homedir(), ".julia", "juliaup", platform))
							if startswith(i, "julia-")
								println("  ", i[7:end], " (", platform, ")")
							end
						end
					end
				end

				defaultJulia = "1"

				julia_config_file_path = joinpath(homedir(), ".julia", "juliaup", "juliaup.toml")

				if isfile(julia_config_file_path)
					config_data = TOML.parsefile(julia_config_file_path)
					defaultJulia = get(config_data, "currentversion", "1")
				end

				println()
				println("The default Julia version is configured to be: ", defaultJulia)
			else
				println("ERROR: The status command does not accept any additional arguments.")
			end
		else
			println("ERROR: '", ARGS[1], "' is not a recognized command.")
		end
	else
		println("Internal error.")
	end

    return
end

if abspath(PROGRAM_FILE) == @__FILE__
    real_main()
end

end
