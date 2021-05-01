#include <string>
#include <vector>
#include <set>
#include <algorithm>

struct JuliaVersion
{
	int major;
	int minor;
	int patch;
public:
	std::string toString() {
		return std::string(std::to_string(this->major) + "." + std::to_string(this->minor) + "." + std::to_string(this->patch));
	}
};

class JuliaVersionsDatabase
{
private:
	void tokenize(std::string& str, char delim, std::vector<std::string>& out)
	{
		size_t start;
		size_t end = 0;

		while ((start = str.find_first_not_of(delim, end)) != std::string::npos)
		{
			end = str.find(delim, start);
			out.push_back(str.substr(start, end - start));
		}
	}

public:
	std::vector<JuliaVersion> getJuliaVersions();

	bool isValidJuliaVersion(std::string_view versionString) {
		auto versions = this->getJuliaVersions();

		return std::any_of(versions.begin(), versions.end(), [&](auto i) {return versionString == i.toString() || versionString == i.toString() + "-x86"; });
	}

	bool isValidJuliaChannel(std::string_view versionString)
	{
		auto versions = this->getJuliaVersions();

		std::set<std::string> channels;

		for (auto const& i : versions) {
			channels.insert(std::to_string(i.major));
			channels.insert(std::to_string(i.major) + "." + std::to_string(i.minor));

			channels.insert(std::to_string(i.major) + "-x86");
			channels.insert(std::to_string(i.major) + "." + std::to_string(i.minor) + "-x86");
		}

		return std::any_of(channels.begin(), channels.end(), [&](auto i) {return i == versionString; });
	}

	std::vector<std::string> getJuliaVersionsThatMatchChannel(std::string channelString) {
		std::vector<std::string> parts;
		tokenize(channelString, L'.', parts);

		std::vector<std::string> versionsThatWeCouldUse;

		auto juliaVersions = this->getJuliaVersions();

		// Collect all the known versions of Julia that exist that match our channel into a vector
		for (int i = juliaVersions.size() - 1; i >= 0; i--) {
			auto& currVersion = juliaVersions[i];
			if (parts.size() == 1 && parts[0] == std::to_string(currVersion.major)) {
				versionsThatWeCouldUse.push_back(currVersion.toString());
			}
			else if (parts.size() == 2 && parts[0] == std::to_string(currVersion.major) && parts[1] == std::to_string(currVersion.minor)) {
				versionsThatWeCouldUse.push_back(currVersion.toString());
			}
		}

		return versionsThatWeCouldUse;
	}
};
