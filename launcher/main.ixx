module;

#include <string>
#include <string_view>
#include <codecvt>
#include <iostream>
#include <ranges>
#include <fstream>
#include <filesystem>
#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Foundation.Collections.h>
#include <winrt/Windows.ApplicationModel.h>
#include <winrt/Windows.Storage.h>
#include "winrt/Windows.Web.Http.h"
#include "winrt/Windows.Storage.Streams.h"
#include "../json/single_include/nlohmann/json.hpp"
#include <windows.h>
#include "version.h"

export module main;

import Tokenizer;
import JuliaVersionDatabase;

using namespace winrt;
using namespace Windows::ApplicationModel;
using namespace Windows::Storage;
using json = nlohmann::json;

std::string GetLastErrorAsString()
{
	//Get the error message, if any.
	DWORD errorMessageID = ::GetLastError();
	if (errorMessageID == 0)
		return std::string(); //No error message has been recorded

	LPSTR messageBuffer = nullptr;
	size_t size = FormatMessageA(FORMAT_MESSAGE_ALLOCATE_BUFFER | FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
		NULL, errorMessageID, MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT), (LPSTR)&messageBuffer, 0, NULL);

	std::string message(messageBuffer, size);

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
	const std::wstring& Argument,
	std::wstring& CommandLine,
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
		Argument.find_first_of(L" \t\n\v\"") == Argument.npos)
	{
		CommandLine.append(Argument);
	}
	else {
		CommandLine.push_back(L'"');

		for (auto It = Argument.begin(); ; ++It) {
			unsigned NumberBackslashes = 0;

			while (It != Argument.end() && *It == L'\\') {
				++It;
				++NumberBackslashes;
			}

			if (It == Argument.end()) {

				//
				// Escape all backslashes, but let the terminating
				// double quotation mark we add below be interpreted
				// as a metacharacter.
				//

				CommandLine.append(NumberBackslashes * 2, L'\\');
				break;
			}
			else if (*It == L'"') {

				//
				// Escape all backslashes and the following
				// double quotation mark.
				//

				CommandLine.append(NumberBackslashes * 2 + 1, L'\\');
				CommandLine.push_back(*It);
			}
			else {

				//
				// Backslashes aren't special here.
				//

				CommandLine.append(NumberBackslashes, L'\\');
				CommandLine.push_back(*It);
			}
		}

		CommandLine.push_back(L'"');
	}
}

HRESULT StartProcess(LPCWSTR applicationName, LPWSTR commandLine, LPCWSTR currentDirectory, DWORD timeout)
{
	STARTUPINFO info;
	GetStartupInfo(&info);

	PROCESS_INFORMATION processInfo{};

	BOOL ret = CreateProcessW(
		applicationName,
		commandLine, //commandLine,
		nullptr, nullptr, // Process/ThreadAttributes
		true, // InheritHandles
		0, //EXTENDED_STARTUPINFO_PRESENT, // CreationFlags
		nullptr, // Environment
		currentDirectory, //currentDirectory,
		//(LPSTARTUPINFO)&startupInfoEx,
		&info,
		&processInfo);

	if (!ret) {
		auto error_message = GetLastErrorAsString();

		printf(error_message.c_str());

		return ERROR;
	}

	// RETURN_HR_IF(HRESULT_FROM_WIN32(ERROR_INVALID_HANDLE), processInfo.hProcess == INVALID_HANDLE_VALUE);
	DWORD waitResult = ::WaitForSingleObject(processInfo.hProcess, timeout);
	// RETURN_LAST_ERROR_IF_MSG(waitResult != WAIT_OBJECT_0, "Waiting operation failed unexpectedly.");
	CloseHandle(processInfo.hProcess);
	CloseHandle(processInfo.hThread);

	return ERROR_SUCCESS;
}

extern "C" IMAGE_DOS_HEADER __ImageBase;

std::wstring GetExecutablePath()
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
			throw std::exception("GetModuleFileName failed"); // You can call GetLastError() to get more info here

		if (GetLastError() != ERROR_INSUFFICIENT_BUFFER)
		{
			buffer.resize(pathLength);
			return buffer;
		}
	}
}

std::wstring getCurrentPlatform() {
#ifdef _M_X64
	return std::wstring{ L"x64" };
#endif

#ifdef _M_IX86
	return std::wstring{ L"x86" };
#endif
}

std::filesystem::path getJuliaupPath() {
	std::filesystem::path homedirPath = std::wstring{ Windows::Storage::UserDataPaths::GetDefault().Profile() };
	return homedirPath / ".julia" / "juliaup";
}

void initial_setup() {
	auto juliaupFolder = getJuliaupPath();

	if (!std::filesystem::exists(juliaupFolder / "juliaup.json")) {

		std::filesystem::path myOwnPath = GetExecutablePath();

		auto pathOfBundledJulia = myOwnPath.parent_path().parent_path() / "BundledJulia";

		auto juliaVersionsDatabase = new JuliaVersionsDatabase();

		auto platform = getCurrentPlatform();

		auto targetPath = juliaupFolder / platform / (L"julia-" + juliaVersionsDatabase->getBundledJuliaVersion());

		std::filesystem::create_directories(targetPath);

		std::filesystem::copy(pathOfBundledJulia, targetPath, std::filesystem::copy_options::overwrite_existing | std::filesystem::copy_options::recursive);

		json j;
		j["Default"] = "1";
		j["InstalledVersions"] = {
			{
				winrt::to_string(juliaVersionsDatabase->getBundledJuliaVersion() + L"~" + platform),
				{
					{"path", winrt::to_string(std::wstring{std::filesystem::path{ L"." } / platform / (L"julia-" + juliaVersionsDatabase->getBundledJuliaVersion())})}
				}
			}
		};

		std::ofstream o(juliaupFolder / "juliaup.json");
		o << std::setw(4) << j << std::endl;
	}
}

winrt::fire_and_forget DownloadVersionDBAsync()
{
	co_await winrt::resume_background();

	Windows::Foundation::Uri uri{ L"https://julialang-s3.julialang.org/bin/versions.json" };

	std::filesystem::path juliaupFolderPath{ std::filesystem::path {std::wstring{ Windows::Storage::UserDataPaths::GetDefault().Profile() } } / ".julia" / "juliaup" };

	Windows::Web::Http::HttpClient httpClient{};

	// Always catch network exceptions for async methods
	try
	{

		auto response{ co_await httpClient.GetAsync(uri) };

		auto buffer{ co_await response.Content().ReadAsBufferAsync() };

		auto folder{ co_await Windows::Storage::StorageFolder::GetFolderFromPathAsync(std::wstring{juliaupFolderPath}) };

		auto file{ co_await folder.CreateFileAsync(L"versions.json", Windows::Storage::CreationCollisionOption::ReplaceExisting) };

		co_await Windows::Storage::FileIO::WriteBufferAsync(file, buffer);
	}
	catch (winrt::hresult_error const& ex)
	{
		// Details in ex.message() and ex.to_abi().
	}
}

export int main(int argc, char* argv[])
{
	init_apartment();

	SetConsoleTitle(L"Julia");

	auto juliaVersionsDatabase = new JuliaVersionsDatabase();

	juliaVersionsDatabase->init(getJuliaupPath());


	DownloadVersionDBAsync();

	initial_setup();


	std::wstring juliaVersionToUse = L"1";
	bool juliaVersionFromCmdLine = false;

	auto configFilePath = getJuliaupPath() / "juliaup.json";

	json configFile;

	if (std::filesystem::exists(configFilePath)) {
		std::ifstream i(configFilePath);
		i >> configFile;
	}
	else
	{
		std::wcout << "ERROR: Could not read the juliaup configuration file." << std::endl;

		return 1;
	}

	juliaVersionToUse = winrt::to_hstring(configFile["/Default"_json_pointer]);

	std::wstring exeArgString = std::wstring{ L"" };

	for (int i = 1; i < argc; i++) {
		std::wstring curr = std::wstring{ winrt::to_hstring(argv[i]) };

		exeArgString.append(L" ");

		if (curr._Starts_with(L"-v=")) {
			juliaVersionToUse = curr.substr(3);
			juliaVersionFromCmdLine = true;
		}
		else {
			ArgvQuote(curr, exeArgString, false);
		}
	}

	std::vector<std::wstring> parts;
	tokenize(juliaVersionToUse, L'~', parts);
	auto& versionPart = parts[0];
	auto platformPart = parts.size() > 1 ? parts[1] : getCurrentPlatform();

	// Now figure out whether we got a channel or a specific version.
	std::vector<std::wstring> parts2;
	tokenize(versionPart, L'.', parts2);

	std::filesystem::path julia_path;

	// We are using a specific Julia version
	if (parts2.size() == 3) {
		json::json_pointer json_path(winrt::to_string(L"/InstalledVersions/" + versionPart + L"~0" + platformPart + L"/path"));

		std::filesystem::path targetPath;

		if (configFile.contains(json_path)) {
			targetPath = getJuliaupPath() / std::wstring{ winrt::to_hstring(configFile[json_path]) };
		}

		if (std::filesystem::exists(targetPath)) {
			julia_path = targetPath / L"bin" / L"julia.exe";
			SetConsoleTitle((L"Julia " + versionPart).c_str());
		}
		else {
			std::wcout << L"Julia version " + juliaVersionToUse + L" is not installed on this system. Run:" << std::endl;
			std::wcout << std::endl;
			std::wcout << L"  juliaup add " + juliaVersionToUse << std::endl;
			std::wcout << std::endl;
			std::wcout << L"to install it." << std::endl;

			return 1;
		}
	}
	// We are using a channel
	else {
		std::vector<std::wstring> versionsThatWeCouldUse;

		auto juliaVersions = juliaVersionsDatabase->getJuliaVersions();

		// Collect all the known versions of Julia that exist that match our channel into a vector
		for (auto& currVersion : std::ranges::reverse_view{ juliaVersions }) {
			if (parts2.size() == 1 && parts2[0] == std::to_wstring(currVersion.major)) {
				auto as_string = currVersion.toString();
				versionsThatWeCouldUse.push_back(std::wstring(as_string.begin(), as_string.end()));
			}
			else if (parts2.size() == 2 && parts2[0] == std::to_wstring(currVersion.major) && parts2[1] == std::to_wstring(currVersion.minor)) {
				auto as_string = currVersion.toString();
				versionsThatWeCouldUse.push_back(std::wstring(as_string.begin(), as_string.end()));
			}
		}

		if (versionsThatWeCouldUse.size() > 0) {
			bool foundLatestJuliaVersionForChannel = false;
			bool foundAnyJuliaVersionForChannel = false;
			std::wstring juliaVersionWeAreUsing;

			for (int i = 0; i < versionsThatWeCouldUse.size(); i++) {
				json::json_pointer json_path(winrt::to_string(L"/InstalledVersions/" + versionsThatWeCouldUse[i] + L"~0" + platformPart + L"/path"));

				std::filesystem::path targetPath;

				if (configFile.contains(json_path)) {
					targetPath = getJuliaupPath() / std::wstring{ winrt::to_hstring(configFile[json_path]) };
				}

				if (std::filesystem::exists(targetPath)) {
					julia_path = targetPath / L"bin" / L"julia.exe";
					foundLatestJuliaVersionForChannel = i == 0;
					foundAnyJuliaVersionForChannel = true;
					juliaVersionWeAreUsing = versionsThatWeCouldUse[i];
					SetConsoleTitle((L"Julia " + versionsThatWeCouldUse[i]).c_str());
					break;
				}
			}

			if (!foundAnyJuliaVersionForChannel) {
				std::wcout << L"No Julia version for channel " + juliaVersionToUse + L" is installed on this system. Run:" << std::endl;
				std::wcout << std::endl;
				if (juliaVersionFromCmdLine)
				{
					std::wcout << L"  juliaup update " << juliaVersionToUse << std::endl;
				}
				else
				{
					std::wcout << L"  juliaup update" << std::endl;
				}
				std::wcout << std::endl;
				std::wcout << L"to install Julia " << versionsThatWeCouldUse[0] << ", the latest Julia version for the " << juliaVersionToUse << " channel." << std::endl;

				return 1;
			}

			if (!foundLatestJuliaVersionForChannel) {
				std::wcout << L"The latest version of Julia in the " << juliaVersionToUse << " channel is Julia " << versionsThatWeCouldUse[0] << ". You currently have Julia " << juliaVersionWeAreUsing << " installed. Run:" << std::endl;
				std::wcout << std::endl;
				if (juliaVersionFromCmdLine)
				{
					std::wcout << L"  juliaup update " << juliaVersionToUse << std::endl;
				}
				else
				{
					std::wcout << L"  juliaup update" << std::endl;
				}
				std::wcout << std::endl;
				std::wcout << L"to install Julia " << versionsThatWeCouldUse[0] << " and update the " << juliaVersionToUse << " channel to that version." << std::endl;
			}
		}
		else {
			std::wcout << L"No Julia versions for channel " + juliaVersionToUse + L" exist. Please select a different channel." << std::endl;
			return 1;
		}
	}

	//std::filesystem::path currentDirectory = L"";

	exeArgString.insert(0, julia_path);

	HRESULT hr = StartProcess(julia_path.c_str(), exeArgString.data(), nullptr, INFINITE);
	if (hr != ERROR_SUCCESS)
	{
		printf("Error return from launching process.");
	}

	return 0;
}
