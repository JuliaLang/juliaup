# Juliaup - Julia version manager

This repository contains a cross-platform installer for the Julia programming language.

The installer also bundles a full Julia version manager called `juliaup`. One can use `juliaup` to install specific Julia versions, it alerts users when new Julia versions are released and provides a convenient Julia release channel abstraction.

## Status

The Windows version of this installer is considered production ready. The Linux and Mac versions are prerelease versions that should not be used in production environments.

## Installation

On all platforms it is recommended that you first uninstall any previous Julia versions and undo any modifications you might have made to put `julia` on the `PATH` before you install Julia with the installer in this repository.

### Windows

On Windows Julia and Juliaup can be installed directly from the Windows store [here](https://www.microsoft.com/store/apps/9NJNWW8PVKMN). One can also install exactly the same version by executing

```
winget install julia -s msstore
```

on a command line.

If the Windows Store is blocked on a system, we have an alternative [MSIX App Installer](https://learn.microsoft.com/en-us/windows/msix/app-installer/app-installer-file-overview) based setup. Note that this is currently experimental, please report back successes and failures [here](https://github.com/JuliaLang/juliaup/issues/343). To use the App Installer version, download [this](https://install.julialang.org/Julia.appinstaller) file and open it by double clicking on it.

### Mac and Linux

Juliaup can be installed on Linux or Mac by executing

```
curl -fsSL https://install.julialang.org | sh
```

in a shell. Note that the Mac and Linux version are considered prerelease, have known bugs and might often break.

If using Juliaup in an automated script, you can skip user prompts by using the `-y/--yes` flag

```
curl -fsSL https://install.julialang.org | sh -s -- -y
```

#### Software Repositories

**Important note:** As of now, we strongly recommend to install Juliaup via the `curl` command above rather than through OS-specific software repositories (see below) as the Juliaup variants provided by the latter currently have some drawbacks (that we hope to lift in the future).

##### [Homebrew](https://brew.sh)

```
brew install juliaup
```

##### [Arch Linux - AUR](https://aur.archlinux.org/packages/juliaup/)

On Arch Linux, Juliaup is available [in the Arch User Repository (AUR)](https://aur.archlinux.org/packages/juliaup/).

##### [openSUSE Tumbleweed](https://get.opensuse.org/tumbleweed/)

On openSUSE Tumbleweed, Juliaup is available. To install, run with root privileges:

```sh
zypper install juliaup
```

## Using Juliaup

Once you have installed Juliaup, `julia` is on the `PATH`, and on Windows there is a start menu shortcut and it will show up as a profile in Windows Terminal. Any of those will start Julia. The VS Code extension will also automatically find this Julia installation.

Here are some of the things you can do with `juliaup`:
- `juliaup list` lists all the available channels.
- `juliaup update` installs the latest available Julia version for all your channels.
- `juliaup update release` updates the `release` channel to the latest version.
- `juliaup status` shows you which Julia versions you have installed and which one is configured as the default.
- `juliaup add 1.5.1` adds Julia 1.5.1 to your system (it can then be launched via the command `julia +1.5.1`).
- `juliaup default 1.5.3` configures the `julia` command to start Julia 1.5.3.
- `juliaup default 1.6` configures the `julia` command to start the latest 1.6.x version of Julia you have installed on your system (and inform you if there is a newer version in 1.6.x available).
- `juliaup default release` configures the `julia` command to start the latest stable version of Julia (this is also the default value).
- `juliaup remove 1.5.3` deletes Julia 1.5.3 from your system.
- `juliaup add 1.6.1~x86` installs the 32 bit version of Julia 1.6.1 on your system.
- `juliaup default 1.6~x86` configures the `julia` command to start the latest 1.6.x 32 bit version of Julia you have installed on your system.
- `juliaup link dev ~/juliasrc/julia` configures the `dev` channel to use a binary that you provide that is located at `~/juliasrc/julia`. You can then use `dev` as if it was a system provided channel, i.e. make it the default or use it with the `+` version selector. You can use other names than `dev` and link as many versions into `juliaup` as you want.
- `juliaup self update` installs the latest version, which is necessary if new releases reach the beta channel, etc.
- `juliaup self uninstall` uninstalls Juliaup. Note that on some platforms this command is not available, in those situations one should use platform specific methods to uninstall Juliaup.
- `juliaup` shows you what other commands are available.

The available system provided channels are:
- `release`: always points to the latest stable version.
- `lts`: always points to the latest long term supported version.
- `beta`: always points to the latest beta version if one exists. If a newer release candidate exists, it will point to that, and if there is neither a beta or rc candidate available it will point to the same version as the `release` channel.
- `rc`: same as `beta`, but only starts with release candidate versions.
- specific versions, e.g. `1.5.4`.
- minor version channels, e.g. `1.5`.
- major version channels, e.g. `1`.

All of these channels can be combined with the `~x86`, `~x64` or `~aarch64` suffix to download a specific platform version.

## Using installed julia versions

To launch the default Julia version simply run `julia` in your terminal.

To launch a specific Julia version, say in channel `release`, run `julia +release`.

## Juliaup server

Juliaup by default downloads julia binary tarballs from the official server "https://julialang-s3.julialang.org".
If requested, the environment variable `JULIAUP_SERVER` can be used to tell Juliaup to use a third-party mirror server.

## Development guides

For juliaup developers, information on how to build juliaup locally, update julia versions, and release updates
can be found in the wiki https://github.com/JuliaLang/juliaup/wiki

To use unstable preview versions of juliaup (e.g. to gt a patch before it makes it into the latest release), use

```
curl -fsSL https://install.julialang.org/releasepreview | sh
```

## More information

[This JuliaCon 2021 talk](https://www.youtube.com/watch?v=rFlbjWC6zYA) is a short introduction to Juliaup. Note that the video was recorded before the Linux and Mac versions were finished, but all the information about `juliaup` itself applies equally on Linux and Mac.

[This JuliaCon 2022 talk](https://www.youtube.com/watch?v=14zfdbzq5BM) provides some background on the design of Juliaup.
