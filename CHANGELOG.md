
# Changelog
<!-- markdownlint-disable MD024 -->

## [1.19.8] - 2/8/2026

### Changed

- Try to use .dmg installers on macOS before .tar.gz.

## [1.19.7] - 2/8/2026

### Changed

- macOS: codesign all Mach-O binaries in PR builds and check notarization.

## [1.19.6] - 2/7/2026

### Changed

- Add codesigning prompt for PR channel updates on macOS and add notarization check.

## [1.19.4] - 1/22/2026

### Changed

- Fix installer crash on permission errors.

## [1.19.3] - 1/19/2026

### Fixed

- Return valid file paths for alias channels in API.

## [1.19.0] - 1/16/2026

### Added

- Add option to match against environment manifest version before using the default channel.
  Opt-in with `juliaup config manifestversiondetect true`.

### Fixed

- Add error handling to notarization.

## [1.18.9] - 10/30/2025

### Fixed

- Make `save_config_db` atomic.
- When failing to update a channel continue to update others.

## [1.18.8] - 10/25/2025

### Changed

- Add space in update message.

## [1.18.7] - 10/23/2025

### Changed

- Update progress bar style to match 1.12 progress bar.
- Conform message styling to intent. Match Pkg formatting.
- Fix some more message colors.
- Update README with juliaup Windows requirements.

## [1.18.5] - 10/19/2025

### Changed

- Revert "Declare "console" subsystem for WindowsApp".
- Add warning and link to PR add mode.
- Query to code sign PR builds on mac.

## [1.18.4] - 10/12/2025

### Changed

- Declare "console" subsystem for WindowsApp.

## [1.18.3] - 10/8/2025

### Changed

- Move check for `is_interactive` earlier.

## [1.18.2] - 10/3/2025

### Fixed

- Don't show `julia` update messages when not interactive.
- Fix `is_interactive` args.

## [1.18.1] - 10/3/2025

### Fixed

- Don't show `julia` update messages when not interactive.

## [1.18.0] - 9/13/2025

### Added

- Offer automatic version installation via channel selection.
- Expanded `link` command to handle aliases and improved test DRYness.
- Colored and wrapped help menu for improved readability.
- Add Nushell completions.

### Fixed

- Fix incorrect TLS and cipher warnings with curl v8.10.0.
- Don't orphan old Julia versions when deletion fails.
- Fix Clippy warnings.

## [1.14.5] - 2/2/2024

### Fixed

- Fix more StoreBroker bugs ([`4fef76f`](https://github.com/JuliaLang/juliaup/commit/4fef76f)) (davidanthoff)

## [1.14.4] - 2/2/2024

### Fixed

- Fix how we force old PowerShell version ([`f94ee4b`](https://github.com/JuliaLang/juliaup/commit/f94ee4b)) (davidanthoff)

## [1.14.3] - 2/2/2024

### Fixed

- Force old PowerShell version on workflow to fix StoreBroker ([`40c954d`](https://github.com/JuliaLang/juliaup/commit/40c954d)) (davidanthoff)

## [1.14.2] - 2/2/2024

### Fixed

- Fix use of direct download StoreBroker ([`40a0d94`](https://github.com/JuliaLang/juliaup/commit/40a0d94)) (davidanthoff)

## [1.14.1] - 2/2/2024

### Fixed

- Use StoreBroker directly from GitHub until new version is released ([`00a39e9`](https://github.com/JuliaLang/juliaup/commit/00a39e9)) (davidanthoff)

## [1.14.0] - 2/2/2024

### Added

- Support nightly channels ([#809](https://github.com/JuliaLang/juliaup/pull/809)) (Roger-luo,maleadt,davidanthoff)

[1.14.0]: https://github.com/JuliaLang/juliaup/releases/tag/v1.14.0
[1.14.1]: https://github.com/JuliaLang/juliaup/releases/tag/v1.14.1
[1.14.2]: https://github.com/JuliaLang/juliaup/releases/tag/v1.14.1
[1.14.3]: https://github.com/JuliaLang/juliaup/releases/tag/v1.14.1
[1.14.4]: https://github.com/JuliaLang/juliaup/releases/tag/v1.14.1
[1.18.0]: https://github.com/JuliaLang/juliaup/releases/tag/v1.18.0
[1.18.1]: https://github.com/JuliaLang/juliaup/releases/tag/v1.18.1
[1.18.2]: https://github.com/JuliaLang/juliaup/releases/tag/v1.18.2
[1.18.3]: https://github.com/JuliaLang/juliaup/releases/tag/v1.18.3
[1.18.4]: https://github.com/JuliaLang/juliaup/releases/tag/v1.18.4
[1.18.5]: https://github.com/JuliaLang/juliaup/releases/tag/v1.18.5
[1.18.7]: https://github.com/JuliaLang/juliaup/releases/tag/v1.18.7
[1.18.8]: https://github.com/JuliaLang/juliaup/releases/tag/v1.18.8
[1.18.9]: https://github.com/JuliaLang/juliaup/releases/tag/v1.18.9
[1.19.0]: https://github.com/JuliaLang/juliaup/releases/tag/v1.19.0
[1.19.3]: https://github.com/JuliaLang/juliaup/releases/tag/v1.19.3
[1.19.4]: https://github.com/JuliaLang/juliaup/releases/tag/v1.19.4
[1.19.6]: https://github.com/JuliaLang/juliaup/releases/tag/v1.19.6
[1.19.7]: https://github.com/JuliaLang/juliaup/releases/tag/v1.19.7
[1.19.8]: https://github.com/JuliaLang/juliaup/releases/tag/v1.19.8
