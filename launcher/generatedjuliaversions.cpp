#include "pch.h"

std::vector<JuliaVersion> JuliaVersionsDatabase::getJuliaVersions() {
	std::vector<JuliaVersion> juliaVersions = { 
    JuliaVersion{1, 5, 1}, JuliaVersion{1, 5, 2}, JuliaVersion{1, 5, 3}, JuliaVersion{1, 5, 4}, JuliaVersion{1, 6, 0}, JuliaVersion{1, 6, 1}
	};
  std::sort(juliaVersions.begin(), juliaVersions.end(), [](const JuliaVersion& a, const JuliaVersion& b) {
		if (a.major == b.major) {
			if (a.minor == b.minor) {
				return a.patch < b.patch;
			}
			else {
				return a.minor < b.minor;
			}
		}
		else {
			return a.major < b.major;
		}
	});
	return juliaVersions;
}
