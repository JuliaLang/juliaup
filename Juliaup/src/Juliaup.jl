module Juliaup

include("Tar/src/Tar.jl")

import Downloads, .Tar, CodecZlib, JSON

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

function get_juliauphome_path()
	if length(Base.DEPOT_PATH)>0
		return joinpath(Base.DEPOT_PATH[1], "juliaup")
	else
		error("No entries in Base.DEPOT_PATH")
	end
end

function get_juliaupconfig_path()
	return joinpath(get_juliauphome_path(), "juliaup.json")
end

function tryparse_full_version(value::AbstractString)
	parts = split(value, '.')
	if length(parts)==3 && !any(i->tryparse(Int, i)===nothing, parts)
		return string(value)
	else
		return nothing
	end
end

function tryparse_channel(value::AbstractString)
	parts = split(value, '.')
	if length(parts)==2 && !any(i->tryparse(Int, i)===nothing, parts)
		return value
	elseif length(parts)==1 && tryparse(Int, value)!==nothing
		return value
	else
		return nothing
	end
end

function try_split_platform(value::AbstractString)
	parts = split(value, '~')
	
	if length(parts)==1
		return (version=value, platform=Int===Int64 ? "x64" : "x86")
	elseif length(parts)==2 && parts[2] in ("x64", "x86")
		return (version=string(parts[1]), platform=string(parts[2]))
	else
		return nothing
	end
end

function get_julia_versions()
	version_db = get_version_db()

	relevant_versions = filter(version_db) do i
		return i[2]["stable"] && any(i[2]["files"]) do j
			return j["os"]=="winnt" && j["kind"]=="archive"
		end
	end

	typed_versions = map(relevant_versions) do i
		return VersionNumber(i[1])
	end

	sort!(typed_versions)

	return typed_versions
end

function getJuliaVersionsThatMatchChannel(channelString)
	parts = split(channelString, '.')

	versionsThatWeCouldUse = VersionNumber[]

	# Collect all the known versions of Julia that exist that match our channel into a vector
	for currVersion in reverse(get_julia_versions())
		if length(parts) == 1 && parts[1] == string(currVersion.major)
			push!(versionsThatWeCouldUse, currVersion)
		elseif length(parts) == 2 && parts[1] == string(currVersion.major) && parts[2] == string(currVersion.minor)
			push!(versionsThatWeCouldUse, currVersion)
		end
	end

	return versionsThatWeCouldUse
end

function installJuliaVersion(platform::String, version::VersionNumber)
	download_url = get_download_url(get_version_db(), version, platform)

	target_path = joinpath(get_juliauphome_path(), platform)

	mkpath(target_path)

	println("Installing Julia $version ($platform).")

	temp_file = Downloads.download(download_url)

	try
		open(temp_file) do tar_gz
			tar = CodecZlib.GzipDecompressorStream(tar_gz)
			try
				mktempdir() do extract_temp_path
					Tar.extract(tar, extract_temp_path, same_permissions=false)
					mv(joinpath(extract_temp_path, "julia-$version"), joinpath(target_path, "julia-$version"), force=true)
				end
			finally
				close(tar)
			end
		end

		juliaup_config_file_path = get_juliaupconfig_path()

		data = isfile(juliaup_config_file_path) ?
			JSON.parsefile(juliaup_config_file_path, use_mmap=false) :
			Dict()

		if !haskey(data, "InstalledVersions")
			data["InstalledVersions"] = Dict()
		end

		data["InstalledVersions"]["$version~$platform"] = Dict("path" => joinpath(".", platform, "julia-$version"))

		open(juliaup_config_file_path, "w") do f
			JSON.print(f, data, 4)
		end

		println("New version successfully installed.")
	finally
		rm(temp_file, force=true)
	end
end

const g_version_db = Ref{Dict}()

function get_version_db()
	if !isassigned(g_version_db)
		version_db_search_paths = [
			joinpath(get_juliauphome_path(), "versions.json"),
			joinpath(Sys.BINDIR, "..", "..", "VersionsDB", "versions.json") # This only exists when MSIX deployed
		]
		for i in version_db_search_paths
			if isfile(i)
				# TODO Remove again
				println("DEBUG: We are using `", i, "` as the version DB file.")
				g_version_db[] = JSON.parsefile(i, use_mmap=false)
				return g_version_db[]
			end
		end

		error("No version database found.")
	else
		return g_version_db[]
	end
end

function get_download_url(version_db, version::VersionNumber, platform::String)
	arch = platform == "x64" ? "x86_64" : platform == "x86" ? "i686" : error("Unknown platform")

	node_for_version = version_db[string(version)]
	node_for_files = node_for_version["files"]

	index_for_files = findfirst(i->i["kind"]=="archive" && i["arch"]==arch && i["os"]=="winnt", node_for_files)

	if index_for_files!==nothing
		zip_url = node_for_files[index_for_files]["url"]

		p1, p2 = splitext(zip_url)

		# TODO Fix this in the version DB
		if p2==".zip"
			return p1 * ".tar.gz"
		else
			return p1
		end
	else
		error("Could not find archive.")
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
		println("  default       Set the default Julia version")
		println("  add           Add a specific Julia version to your system")
		println("  update        Update the current or a specific channel to the latest Julia version")
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
		elseif ARGS[1] == "default"
			if length(ARGS)==2
				first_split = try_split_platform(ARGS[2])
				if first_split!==nothing && (tryparse_full_version(first_split.version)!==nothing || tryparse_channel(first_split.version)!==nothing)
					juliaup_config_file_path = get_juliaupconfig_path()

					data = isfile(juliaup_config_file_path) ?
						JSON.parsefile(juliaup_config_file_path, use_mmap=false) :
						Dict()

					data["Default"] = ARGS[2]

					open(juliaup_config_file_path, "w") do f
						JSON.print(f, data, 4)
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
				first_split = try_split_platform(ARGS[2])
				if first_split!==nothing && tryparse_full_version(first_split.version)!==nothing
					version_to_install = VersionNumber(first_split.version)

					installJuliaVersion(first_split.platform, version_to_install)
				else
					# TODO Come up with a less hardcoded version of this.
					println("ERROR: '", ARGS[2], "' is not a valid Julia version. Valid values are '1.5.1', '1.5.2', '1.5.3', '1.5.4', '1.6.0' or '1.6.1'.")
				end
			else
				println("ERROR: The add command only accepts one additional argument.")
			end
		elseif ARGS[1] == "update" || ARGS[1] == "up"
			if length(ARGS)==1
				julia_config_file_path = get_juliaupconfig_path()

				if isfile(julia_config_file_path)
					config_data = JSON.parsefile(julia_config_file_path, use_mmap=false)
					juliaVersionToUse = get(config_data, "Default", "1")

					first_split = try_split_platform(juliaVersionToUse)

					if first_split!==nothing
						if tryparse_channel(first_split.version)!==nothing
							publishedVersionsWeCouldUse = getJuliaVersionsThatMatchChannel(first_split.version)

							if length(publishedVersionsWeCouldUse) > 0
									if haskey(get(config_data, "InstalledVersions", Dict()), "$(publishedVersionsWeCouldUse[1])~$(first_split.platform)")
										println("You already have the latest Julia version for the active channel installed.")
									else
										installJuliaVersion(first_split.platform, publishedVersionsWeCouldUse[1])
									end
							else
								println("You currently have a Julia channel configured for which no Julia versions exists. Nothing can be updated.")
							end
						elseif tryparse_full_version(first_split.version)!==nothing
							println("You currently have a specific Julia version as your default configured. Only channel defaults can be updated.")
						else
							println("ERROR: The configuration value for `currentversion` is invalid.")
						end
					else
						println("ERROR: The configuration value for `currentversion` is invalid.")
					end
				else
					println("ERROR: Could not find the juliaup configuration file.")
				end
			elseif length(ARGS)==2
				julia_config_file_path = get_juliaupconfig_path()

				if isfile(julia_config_file_path)
					config_data = JSON.parsefile(julia_config_file_path, use_mmap=false)
					juliaVersionToUse = ARGS[2]

					first_split = try_split_platform(juliaVersionToUse)

					if first_split!==nothing
						if tryparse_channel(first_split.version)!==nothing
							publishedVersionsWeCouldUse = getJuliaVersionsThatMatchChannel(first_split.version)

							if length(publishedVersionsWeCouldUse) > 0
									if haskey(get(config_data, "InstalledVersions", Dict()), "$(publishedVersionsWeCouldUse[1])~$(first_split.platform)")
										println("You already have the latest Julia version for the $juliaVersionToUse channel installed.")
									else
										installJuliaVersion(first_split.platform, publishedVersionsWeCouldUse[1])
									end
							else
								println("You are trying to update a Julia channel for which no Julia versions exists. Nothing can be updated.")
							end
						else
							println("ERROR: The argument to the `update` command is invalid.")
						end
					else
						println("ERROR: The argument to the `update` command is invalid.")
					end
				else
					println("ERROR: Could not find the juliaup configuration file.")
				end
			else
				println("ERROR: The update command accepts at most one additional argument.")
			end
		elseif ARGS[1] == "remove" || ARGS[1] == "rm"			

			if length(ARGS)==2

				first_split = try_split_platform(ARGS[2])

				if first_split!==nothing && tryparse_full_version(first_split.version)!==nothing
					juliaVersionToUninstall = first_split.version

					juliaup_config_file_path = get_juliaupconfig_path()

					if isfile(juliaup_config_file_path)
						config_data = JSON.parsefile(juliaup_config_file_path, use_mmap=false)
						node_for_version = get(get(config_data, "InstalledVersions", Dict()), "$juliaVersionToUninstall~$(first_split.platform)", nothing)

						if node_for_version!==nothing
							if haskey(node_for_version, "path")
								path_to_be_deleted = joinpath(get_juliauphome_path(), node_for_version["path"])

								if isdir(path_to_be_deleted)
									rm(path_to_be_deleted, force=true, recursive=true)

									delete!(config_data["InstalledVersions"], "$juliaVersionToUninstall~$(first_split.platform)")

									open(juliaup_config_file_path, "w") do f
										JSON.print(f, config_data, 4)
									end

									println("Julia $juliaVersionToUninstall successfully removed.")
								else
									println("Julia $juliaVersionToUninstall cannot be removed because it is currently not installed.")
								end
							else
								println("ERROR: juliaup.json is misconfigured.")
							end
						else
							println("ERROR: Julia $juliaVersionToUninstall ($(first_split.platform)) is not installed.")
						end
					else
						println("ERROR: Could not find juliaup configuration file.")
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
				julia_config_file_path = get_juliaupconfig_path()

				if isfile(julia_config_file_path)
					config_data = JSON.parsefile(julia_config_file_path, use_mmap=false)

					defaultJulia = get(config_data, "Default", "1")

					println("The following Julia versions are currently installed:")

					for i in get(config_data, "InstalledVersions", Dict())
						version, platform = try_split_platform(i[1])
						println("  $version ($platform)")
					end

					println()
					println("The default Julia version is configured to be: ", defaultJulia)
				else
					println("ERROR: Could not find the juliaup configuration file.")
				end
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
