#include <string>
#include <string_view>
#include <codecvt>
#include <iostream>
#include <ranges>
#include <fstream>
#include <filesystem>
#include "../json/single_include/nlohmann/json.hpp"
#include <windows.h>
#include <userenv.h>
#include "version.h"
#include <exception>

using std::string;
using std::filesystem::path;
using std::cout;
using std::runtime_error;
using json = nlohmann::json;

class JuliaupUserError : public std::runtime_error
{
	std::string what_message;
public:
	JuliaupUserError(string msg) : std::runtime_error(msg)
	{

	}
};

class JuliaupConfigError : public std::runtime_error
{
	std::string what_message;
public:
	JuliaupConfigError(string msg) : std::runtime_error(msg)
	{

	}
};

string GetLastErrorAsString()
{
	//Get the error message, if any.
	DWORD errorMessageID = ::GetLastError();
	if (errorMessageID == 0)
		return string(); //No error message has been recorded

	LPSTR messageBuffer = nullptr;
	size_t size = FormatMessageA(FORMAT_MESSAGE_ALLOCATE_BUFFER | FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
		NULL, errorMessageID, MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT), (LPSTR)&messageBuffer, 0, NULL);

	string message(messageBuffer, size);

	//Free the buffer.
	LocalFree(messageBuffer);

	return message;
}

/*++

Routine Description:

	This routine appends the given argument to a command line such
	that CommandLineToArgvW will return the argument string unchanged.
	Arguments in a command line should be separated by spaces; this
	function does not add these spaces.

Arguments:

	Argument - Supplies the argument to encode.

	CommandLine - Supplies the command line to which we append the encoded argument string.

	Force - Supplies an indication of whether we should quote
			the argument even if it does not contain any characters that would
			ordinarily require quoting.

Return Value:

	None.

Environment:

	Arbitrary.

This function was copied from https://web.archive.org/web/20190109172835/https://blogs.msdn.microsoft.com/twistylittlepassagesallalike/2011/04/23/everyone-quotes-command-line-arguments-the-wrong-way/
on 6/7/2021 by David Anthoff.
--*/
void ArgvQuote(
	const string& Argument,
	string& CommandLine,
	bool Force
)
{
	//
	// Unless we're told otherwise, don't quote unless we actually
	// need to do so --- hopefully avoid problems if programs won't
	// parse quotes properly
	//

	if (Force == false &&
		Argument.empty() == false &&
		Argument.find_first_of(" \t\n\v\"") == Argument.npos)
	{
		CommandLine.append(Argument);
	}
	else {
		CommandLine.push_back('"');

		for (auto It = Argument.begin(); ; ++It) {
			unsigned NumberBackslashes = 0;

			while (It != Argument.end() && *It == '\\') {
				++It;
				++NumberBackslashes;
			}

			if (It == Argument.end()) {

				//
				// Escape all backslashes, but let the terminating
				// double quotation mark we add below be interpreted
				// as a metacharacter.
				//

				CommandLine.append(NumberBackslashes * 2, '\\');
				break;
			}
			else if (*It == '"') {

				//
				// Escape all backslashes and the following
				// double quotation mark.
				//

				CommandLine.append(NumberBackslashes * 2 + 1, '\\');
				CommandLine.push_back(*It);
			}
			else {

				//
				// Backslashes aren't special here.
				//

				CommandLine.append(NumberBackslashes, '\\');
				CommandLine.push_back(*It);
			}
		}

		CommandLine.push_back('"');
	}
}

void CheckRet(BOOL ret)
{
	if (!ret) {
		auto error_message = GetLastErrorAsString();

		throw runtime_error(error_message);
	}
	return;
}

int StartProcess(string commandLine)
{
	STARTUPINFOA info;
	GetStartupInfoA(&info);
	PROCESS_INFORMATION processInfo{};

	BOOL ret;

	ret = CreateProcessA(
		nullptr,
		&commandLine[0], //commandLine,
		nullptr, nullptr, // Process/ThreadAttributes
		true, // InheritHandles
		0, //EXTENDED_STARTUPINFO_PRESENT, // CreationFlags
		nullptr, // Environment
		nullptr, //currentDirectory,
		//(LPSTARTUPINFO)&startupInfoEx,
		&info,
		&processInfo);
	CheckRet(ret);
	if (processInfo.hProcess == INVALID_HANDLE_VALUE)
	{
		throw runtime_error("Invalid handle.");
	}

	DWORD waitResult = ::WaitForSingleObject(processInfo.hProcess, INFINITE);
	if (waitResult != WAIT_OBJECT_0)
	{
		auto error_message = GetLastErrorAsString();

		throw runtime_error(error_message);
	}

	DWORD exitCode = 0;
	ret = GetExitCodeProcess(processInfo.hProcess, &exitCode);
	CheckRet(ret);

	ret = CloseHandle(processInfo.hProcess);
	CheckRet(ret);

	ret = CloseHandle(processInfo.hThread);
	CheckRet(ret);

	return exitCode;
}

extern "C" IMAGE_DOS_HEADER __ImageBase;

path GetExecutablePath()
{
	std::wstring buffer;
	size_t nextBufferLength = MAX_PATH;

	for (;;)
	{
		buffer.resize(nextBufferLength);
		nextBufferLength *= 2;

		SetLastError(ERROR_SUCCESS);

		auto pathLength = GetModuleFileName(reinterpret_cast<HMODULE>(&__ImageBase), &buffer[0], static_cast<DWORD>(buffer.length()));

		if (pathLength == 0)
			throw runtime_error("GetModuleFileName failed"); // You can call GetLastError() to get more info here

		if (GetLastError() != ERROR_INSUFFICIENT_BUFFER)
		{
			buffer.resize(pathLength);
			return buffer;
		}
	}
}

string GetCurrentPlatform() {
#ifdef _M_X64
	return "x64";
#endif

#ifdef _M_IX86
	return "x86";
#endif
}

path GetHomedirPath() {
	BOOL ret;

	HANDLE token = INVALID_HANDLE_VALUE;
	ret = OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &token);
	CheckRet(ret);

	string buffer;
	buffer.resize(MAX_PATH);
	DWORD pathLen = MAX_PATH;
	
	ret = GetUserProfileDirectoryA(token, &buffer[0], &pathLen);
	CheckRet(ret);

	// Strip the trailing null character.
	buffer.resize(pathLen > 0 ? pathLen - 1 : 0);

	return buffer;
}

path GetJuliaupPath() {
	path homedirPath{ GetHomedirPath() };
	return homedirPath / ".julia" / "juliaup";
}

void DoCleanupOfOldVersions()
{
	auto juliaupFolder = GetJuliaupPath();

	try
	{
		if (std::filesystem::exists(juliaupFolder / "juliaup.toml"))
		{
			std::filesystem::remove(juliaupFolder / "juliaup.toml");
		}

		if (std::filesystem::exists(juliaupFolder / "x64"))
		{
			std::filesystem::remove_all(juliaupFolder / "x64");
		}

		if (std::filesystem::exists(juliaupFolder / "x86"))
		{
			std::filesystem::remove_all(juliaupFolder / "x86");
		}
	}
	catch(std::filesystem::filesystem_error& err)
	{ 
		cout << "WARNING: Something went wrong during cleanup of old versions. Details: " << err.what() << std::endl;
	}
}

void DoInitialSetup()
{
	auto juliaupFolder = GetJuliaupPath();

	if (!std::filesystem::exists(juliaupFolder / "juliaup.json")) {

		path myOwnPath = GetExecutablePath();

		auto pathOfBundledJulia = myOwnPath.parent_path().parent_path() / "BundledJulia";

		string bundledVersion{ JULIA_APP_BUNDLED_JULIA };

		auto platform = GetCurrentPlatform();

		auto targetFolderName = "julia-" + bundledVersion + "~" + platform;

		auto targetPath = juliaupFolder / targetFolderName;

		std::filesystem::create_directories(targetPath);

		std::filesystem::copy(pathOfBundledJulia, targetPath, std::filesystem::copy_options::overwrite_existing | std::filesystem::copy_options::recursive);

		json j;
		j["Default"] = "release";
		j["InstalledVersions"] = {
			{
				bundledVersion + "~" + platform,
				{
					{"Path", (path{ "." } / targetFolderName).string()}
				}
			}
		};
		j["InstalledChannels"] = {
			{
				string{"release"},
				{
					{"Version", bundledVersion + "~" + platform}
				}
			}
		};

		std::ofstream o(juliaupFolder / "juliaup.json");
		o << std::setw(4) << j << std::endl;
	}
}

//winrt::fire_and_forget DownloadVersionDBAsync()
//{
//	co_await winrt::resume_background();
//
//	Windows::Foundation::Uri uri{ "https://www.david-anthoff.com/juliaup-versionsdb-winnt-" + getCurrentPlatform() + ".json" };
//
//	path juliaupFolderPath{ path {to_string(Windows::Storage::UserDataPaths::GetDefault().Profile()) } / ".julia" / "juliaup" };
//
//	Windows::Web::Http::HttpClient httpClient{};
//
//	// Always catch network exceptions for async methods
//	try
//	{
//
//		auto response{ co_await httpClient.GetAsync(uri) };
//
//		auto buffer{ co_await response.Content().ReadAsBufferAsync() };
//
//		auto folder{ co_await Windows::Storage::StorageFolder::GetFolderFromPathAsync(juliaupFolderPath) };
//
//		auto file{ co_await folder.CreateFileAsync(to_hstring("juliaup-versionsdb-winnt-" + getCurrentPlatform() + ".json"), Windows::Storage::CreationCollisionOption::ReplaceExisting) };
//
//		co_await Windows::Storage::FileIO::WriteBufferAsync(file, buffer);
//	}
//	catch (winrt::hresult_error const& ex)
//	{
//		// Details in ex.message() and ex.to_abi().
//	}
//}

path GetJuliaupconfigPath()
{
	return GetJuliaupPath() / "juliaup.json";
}

json LoadVersionsDB()
{
	auto currentPlatform{ GetCurrentPlatform() };
	path versionsDBFilename{ "juliaup-versionsdb-winnt-" + currentPlatform + ".json" };

	std::vector<path> version_db_search_paths{
		GetJuliaupPath() / versionsDBFilename,
		GetExecutablePath().parent_path().parent_path() / "VersionsDB" / versionsDBFilename
	};

	for (auto& i : version_db_search_paths) {
		if (std::filesystem::exists(i)) {
			std::ifstream file(i);

			json versiondbData;

			try
			{
				file >> versiondbData;

				return versiondbData;
			}
			catch (json::parse_error& err)
			{
				throw JuliaupConfigError("The versions database file is not a valid JSON file (`" + string{ err.what() } + "`).");
			}
		}
	}

	throw runtime_error("Could not find any versions database.");
}

json LoadConfigDB()
{
	auto configFilePath{ GetJuliaupconfigPath() };

	if (std::filesystem::exists(configFilePath)) {
		std::ifstream i(configFilePath);
		json configFile;

		try
		{
			i >> configFile;

			return configFile;
		}
		catch (json::parse_error& err)
		{
			throw JuliaupConfigError("The juliaup configuration file is not a valid JSON file (`" + string{ err.what() } + "`).");
		}
	}
	else
	{
		throw JuliaupConfigError("Could not read configuration file at `" + configFilePath.string() + "`.");
	}
}

void CheckChannelUptodate(string channel, string currentVersion, json versionsDB)
{
	if (!versionsDB.contains("AvailableChannels"))
	{
		throw JuliaupConfigError("Could not find `AvailableChannels` element in versions database.");
	}

	if (!versionsDB["AvailableChannels"].contains(channel))
	{
		throw JuliaupConfigError("The configured channel `" + channel + "` does not exist in the versions database.");
	}

	if (!versionsDB["AvailableChannels"][channel].contains("Version"))
	{
		throw JuliaupConfigError("The `Version` element is missing for channel `" + channel + "` in the versions database.");
	}

	auto latestVersion{ versionsDB["AvailableChannels"][channel]["Version"].get<string>() };

	if (latestVersion != currentVersion) {
		cout << "The latest version of Julia in the `" << channel << "` channel is " << latestVersion << ". You currently have " << currentVersion << " installed. Run:" << std::endl;
		cout << std::endl;
		cout << "  juliaup update" << std::endl;
		cout << std::endl;
		cout << "to install Julia " << latestVersion << " and update the `" << channel << "` channel to that version." << std::endl;
	}
}

path GetJuliaPathFromChannel(json versionsDB, json configDB, string channel, path juliaupConfigPath, bool channelIsFromConfig)
{
	if (!configDB.contains("InstalledChannels"))
	{
		throw JuliaupConfigError("The `InstalledChannels` element is missing from the juliaup configuration file.");
	}

	if (!configDB["InstalledChannels"].contains(channel))
	{
		if (channelIsFromConfig)
		{
			throw JuliaupConfigError("No channel with name `" + channel + "` exists in the juliaup configuration file.");
		}
		else
		{
			throw JuliaupUserError("No channel named `" + channel + "` exists. Please use the name of an installed channel.");
		}
	}

	if (configDB["InstalledChannels"][channel].contains("Command")) {
		return configDB["InstalledChannels"][channel]["Command"].get<string>();
	}

	if (!configDB["InstalledChannels"][channel].contains("Version")) {
		throw JuliaupConfigError("The juliaup configuration has neither a `Command` nor a `Version` element for channel `" + channel + "`.");
	}

	auto version = configDB["InstalledChannels"][channel]["Version"].get<string>();

	if (!configDB.contains("InstalledVersions"))
	{
		throw JuliaupConfigError("The juliaup configuration file is missing the `InstalledVersions` element.");
	}

	if (!configDB["InstalledVersions"].contains(version))
	{
		throw JuliaupConfigError("The channel `" + channel + "` points to a Julia version that is not installed.");
	}

	if (!configDB["InstalledVersions"][version].contains("Path")) {
		throw JuliaupConfigError("The juliaup configuration for version `" + version + "` is missing a `Path` element.");
	}

	CheckChannelUptodate(channel, version, versionsDB);

	auto absolutePath = juliaupConfigPath.parent_path() / configDB["InstalledVersions"][version]["Path"].get<string>() / "bin" / "julia.exe";

	auto normalizedPath{ absolutePath.lexically_normal() };

	return normalizedPath;
}

int main(int argc, char* argv[])
{
	SetConsoleTitle(L"Julia");

	try
	{

		auto juliaupConfigPath{ GetJuliaupconfigPath() };

		DoInitialSetup();

		DoCleanupOfOldVersions();

		json versionsDB{ LoadVersionsDB() };

		json configDB{ LoadConfigDB() };

		if (!configDB.contains("Default"))
		{
			throw runtime_error("The juliaup configuration file is missing the `Default` element.");
		}
		string juliaChannelToUse{ configDB["Default"].get<string>() };

		string exeArgString{ "" };
		bool juliaVersionFromCmdLine = false;
		for (int i = 1; i < argc; i++) {
			string curr{ argv[i] };

			exeArgString.append(" ");

			if (i == 1 && curr._Starts_with("+")) {
				juliaChannelToUse = curr.substr(1);
				juliaVersionFromCmdLine = true;
			}
			else {
				ArgvQuote(curr, exeArgString, false);
			}
		}

		path julia_path{ GetJuliaPathFromChannel(versionsDB, configDB, juliaChannelToUse, juliaupConfigPath, !juliaVersionFromCmdLine) };

		exeArgString.insert(0, julia_path.string());

		int result = StartProcess(exeArgString);

		return result;
	}
	catch (JuliaupConfigError& err)
	{
		cout << "ERROR: Configuration corrupted. " << err.what() << std::endl;
		return 1;
	}
	catch (JuliaupUserError& err)
	{
		cout << "ERROR: Invalid input. " << err.what() << std::endl;
		return 1;
	}
}
