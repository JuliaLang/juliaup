module Juliaup

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
			# if (__argc == 3) {
			# 	auto secondArg = std::string_view{ __argv[2] };

			# 	if (juliaVersions->isValidJuliaVersion(secondArg) || juliaVersions->isValidJuliaChannel(secondArg)) {
			# 		localSettings.Values().Insert(L"version", box_value(winrt::to_hstring(secondArg)));

			# 		println("Configured the default Julia version to be " << secondArg << "." << std::endl;
			# 	}				
			# 	else {
			# 		// TODO Come up with a less hardcoded version of this.
			# 		println("ERROR: '" << secondArg << "' is not a valid Julia version. Valid values are '1.5.1', '1.5.2', '1.5.3', '1.5.4', '1.6.0' or '1.6.1'." << std::endl;
			# 	}
			# }
			# else {
			# 	println("ERROR: The setdefault command only accepts one additional argument." << std::endl;
			# }
		elseif ARGS[1] == "add"
			# if (__argc == 3) {
			# 	auto secondArg = std::string(__argv[2]);

			# 	if (juliaVersions->isValidJuliaVersion(secondArg)) {

			# 		/*auto catalog = PackageCatalog::OpenForCurrentPackage();

			# 		std::vector<std::string> parts;
			# 		tokenize(secondArg, '-', parts);
					
			# 		auto packageToInstall = std::string("Julia-") + (parts.size()==2 ? (parts[1] + "-" + parts[0]) : secondArg) + "_b0ra4bp6jsp6c";

			# 		println("Installing Julia " << secondArg << "." << std::endl;

			# 		auto res = catalog.AddOptionalPackageAsync(winrt::to_hstring(packageToInstall)).get();

			# 		auto ext_err = res.ExtendedError();

			# 		if (ext_err == 0) {
			# 			println("New version successfully installed." << std::endl;
			# 		}
			# 		else {
			# 			auto err = hresult_error(ext_err);

			# 			std::wcout << err.message().c_str() << std::endl;
			# 		}*/
			# 	}
			# 	else {
			# 		// TODO Come up with a less hardcoded version of this.
			# 		println("ERROR: '" << secondArg << "' is not a valid Julia version. Valid values are '1.5.1', '1.5.2', '1.5.3', '1.5.4', '1.6.0' or '1.6.1'." << std::endl;
			# 	}
			# }
			# else {
			# 	println("ERROR: The add command only accepts one additional argument." << std::endl;
			# }
		elseif ARGS[1] == "update" || ARGS[1] == "up"
			# if length(ARGS)==1
			# 	//std::string juliaVersionToUse = "1";

			# 	//if (localSettings.Values().HasKey(L"version")) {
			# 	//	juliaVersionToUse = to_string(unbox_value<winrt::hstring>(localSettings.Values().Lookup(L"version")));
			# 	//}

			# 	//std::vector<std::string> parts;
			# 	//tokenize(juliaVersionToUse, '-', parts);
			# 	//auto& versionPart = parts[0];
			# 	//auto platformPart = parts.size() > 1 ? parts[1] : "";

			# 	//// Now figure out whether we got a channel or a specific version.
			# 	//std::vector<std::string> parts2;
			# 	//tokenize(versionPart, '.', parts2);

			# 	//if (parts2.size() < 3) {
			# 	//	auto publishedVersionsWeCouldUse = juliaVersions->getJuliaVersionsThatMatchChannel(versionPart);

			# 	//	if (publishedVersionsWeCouldUse.size() > 0) {
			# 	//		auto catalog = PackageCatalog::OpenForCurrentPackage();

			# 	//		auto fullVersionString = (parts.size() == 2 ? (parts[1] + "-" + publishedVersionsWeCouldUse[0]) : publishedVersionsWeCouldUse[0]);
			# 	//		auto fullVersionStringNice = (parts.size() == 2 ? (publishedVersionsWeCouldUse[0] + "-" + parts[1]) : publishedVersionsWeCouldUse[0]);

			# 	//		auto packageToInstall = std::string("Julia-") + fullVersionString + "_b0ra4bp6jsp6c";

			# 	//		println("Installing Julia " << fullVersionStringNice << "." << std::endl;

			# 	//		auto res = catalog.AddOptionalPackageAsync(winrt::to_hstring(packageToInstall)).get();

			# 	//		auto ext_err = res.ExtendedError();

			# 	//		if (ext_err == 0) {
			# 	//			println("New version successfully installed." << std::endl;
			# 	//		}
			# 	//		else {
			# 	//			auto err = hresult_error(ext_err);

			# 	//			std::wcout << err.message().c_str() << std::endl;
			# 	//		}
			# 	//	}
			# 	//	else {
			# 	//		println("You currently have a Julia channel configured for which no Julia versions exists. Nothing can be updated." << std::endl;
			# 	//	}
			# 	//}
			# 	//else {
			# 	//	println("You currently have a specific Julia version as your default configured. Only channel defaults can be updated." << std::endl;
			# 	//}
			# }
			# else {
			# 	println("ERROR: The update command does not accept any additional arguments." << std::endl;
			# }
		elseif ARGS[1] == "remove" || ARGS[1] == "rm"
			# if (__argc == 3) {
			# 	auto secondArg = std::string(__argv[2]);

			# 	if (juliaVersions->isValidJuliaVersion(secondArg)) {
			# 		/*auto juliaVersionToUninstall = secondArg;

			# 		std::vector<std::string> parts;
			# 		tokenize(juliaVersionToUninstall, L'-', parts);

			# 		auto formattedJuliaVersionToUninstall = parts.size() == 1 ? parts[0] : parts[1] + "-" + parts[0];

			# 		auto allInstalledDeps = Package::Current().Dependencies();

			# 		bool foundJuliaVersion = false;

			# 		for (auto v : allInstalledDeps) {
			# 			std::wstring name{ v.Id().Name() };

			# 			if (name == L"Julia-" + winrt::to_hstring(formattedJuliaVersionToUninstall)) {
			# 				foundJuliaVersion = true;
			# 				break;
			# 			}
			# 		}

			# 		if (foundJuliaVersion) {
			# 			auto packagesToUninstall{ winrt::single_threaded_vector<winrt::hstring>({ winrt::to_hstring(std::string("Julia-") + formattedJuliaVersionToUninstall + "_b0ra4bp6jsp6c") }) };

			# 			auto catalog = PackageCatalog::OpenForCurrentPackage();

			# 			auto res = catalog.RemoveOptionalPackagesAsync(packagesToUninstall).get();

			# 			auto ext_err = res.ExtendedError();

			# 			if (ext_err == NULL) {
			# 				println("Julia " + juliaVersionToUninstall + " successfully removed." << std::endl;
			# 			}
			# 			else {
			# 				auto err = hresult_error(ext_err);

			# 				std::wcout << err.message().c_str() << std::endl;
			# 			}
			# 		}
			# 		else {
			# 			println("Julia " + juliaVersionToUninstall  + " cannot be removed because it is currently not installed." << std::endl;
			# 		}*/
			# 	}
			# 	else {
			# 		// TODO Come up with a less hardcoded version of this.
			# 		println("ERROR: '" << secondArg << "' is not a valid Julia version. Valid values are '1.5.1', '1.5.2', '1.5.3', '1.5.4', '1.6.0' or '1.6.1'." << std::endl;
			# 	}
			# }
			# else {
			# 	println("ERROR: The remove command only accepts one additional argument." << std::endl;
			# }
		elseif ARGS[1] == "status" || ARGS[1] == "st"
			# if length(ARGS)==1
			# 	println("The following Julia versions are currently installed:" << std::endl;

			# 	/*auto allInstalledDeps = Package::Current().Dependencies();

			# 	for (auto v : allInstalledDeps) {
			# 		std::wstring name{ v.Id().Name() };

			# 		if (name.starts_with(L"Julia-")) {
			# 			std::wcout << L"  " << name << std::endl;
			# 		}
			# 	}*/

			# }
			# else {
			# 	println("ERROR: The status command does not accept any additional arguments." << std::endl;
			# }
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
