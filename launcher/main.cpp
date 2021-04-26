#include "pch.h"

using namespace winrt;
using namespace Windows::ApplicationModel;
using namespace Windows::Storage;

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

HRESULT StartProcess(LPCWSTR applicationName, LPWSTR commandLine, LPCWSTR currentDirectory, DWORD timeout)
{
	STARTUPINFO info;
	GetStartupInfo(&info);

	PROCESS_INFORMATION processInfo{};

	BOOL ret = CreateProcessW(
		applicationName,
		GetCommandLineW(), //commandLine,
		nullptr, nullptr, // Process/ThreadAttributes
		true, // InheritHandles
		0, //EXTENDED_STARTUPINFO_PRESENT, // CreationFlags
		nullptr, // Environment
		nullptr, //currentDirectory,
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

int wmain(int argc, wchar_t* argv[], wchar_t* envp[]) {
	init_apartment();

	auto localSettings = ApplicationData::Current().LocalSettings();

	std::wstring juliaVersionToUse = L"1.6.1";

	if (localSettings.Values().HasKey(L"version")) {
		juliaVersionToUse = unbox_value<winrt::hstring>(localSettings.Values().Lookup(L"version"));
	}

	auto allInstalledDeps = Package::Current().Dependencies();

	winrt::hstring julia_path;

	bool foundJuliaVersion = false;

	for (auto v : allInstalledDeps) {
		std::wstring name{ v.Id().Name() };

		if (name == L"Julia-" + juliaVersionToUse) {
			auto juliaBinaryStorageLocation = v.InstalledLocation().GetFileAsync(L"Julia\\bin\\julia.exe").get();
			julia_path = juliaBinaryStorageLocation.Path();
			foundJuliaVersion = true;
			break;
		}
	}

	if (!foundJuliaVersion) {
		std::wcout << L"Julia version " + juliaVersionToUse + L" is not installed on this system. Run:" << std::endl;
		std::wcout << std::endl;
		std::wcout << L"  juliaup add " + juliaVersionToUse << std::endl;
		std::wcout << std::endl;
		std::wcout << L"to install it." << std::endl;

		return 1;
	}

	/*for (auto i : Package::Current().Dependencies()) {
		std::wcout << L"NEXT PKG: " << i.Id().FamilyName().c_str() << std::endl;
	}*/

	std::filesystem::path exePath = std::wstring{ julia_path };
	std::filesystem::path currentDirectory = L"";
	std::wstring exeArgString = (wchar_t*)L"";

	std::wstring fullargs = (L"\"" + exePath.native() + L"\" " + exeArgString + L" "); // +args);

	fullargs = L"";
	HRESULT hr = StartProcess(exePath.c_str(), fullargs.data(), currentDirectory.c_str(), INFINITE);
	if (hr != ERROR_SUCCESS)
	{
		printf("Error return from launching process.");
	}

	return 0;
}
