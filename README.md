# Juliaup - Julia version manager

This repository contains a cross-platform installer for the Julia programming language.

The installer also bundles a full Julia version manager called `juliaup`. One can use `juliaup` to install specific Julia versions, it alerts users when new Julia versions are released and provides a convenient Julia release channel abstraction.

## Status

This installer is considered production ready.

## Installation

On all platforms it is recommended that you first uninstall any previous Julia versions and undo any modifications you might have made to put `julia` on the `PATH` before you install Julia with the installer in this repository.

### Windows

On Windows Julia and Juliaup can be installed directly from the Windows store [here](https://www.microsoft.com/store/apps/9NJNWW8PVKMN). One can also install exactly the same version by executing

```
winget install --name Julia --id 9NJNWW8PVKMN -e -s msstore
```

on a command line.

If the Windows Store is blocked on a system, we have an alternative [MSIX App Installer](https://learn.microsoft.com/en-us/windows/msix/app-installer/app-installer-file-overview) based setup. Note that this is currently experimental, please report back successes and failures [here](https://github.com/JuliaLang/juliaup/issues/343). To use the App Installer version, download [this](https://install.julialang.org/Julia.appinstaller) file and open it by double clicking on it.

If neither the Windows Store nor the App Installer version work on your Windows system, you can also use a MSI based installer. Note that this installation methods comes with serious limitations and is generally not recommended unless no other method works. For example, there is no automatic update mechanism for Juliaup with this installation method. The 64 bit version of the MSI installer can be downloaded from [here](https://install.julialang.org/Julia-x64.msi) and the 32 bit version from [here](https://install.julialang.org/Julia-x86.msi). By default the install will be a per-user install that does not require elevation. You can also do a system install by running the following command from a shell:

```
msiexec /i <PATH_TO_JULIA_MSI> ALLUSERS=1
```

### Mac, Linux, and FreeBSD

Juliaup can be installed on Unix-like platforms (currently Linux, Mac, or FreeBSD) by executing

```
curl -fsSL https://install.julialang.org | sh
```

in a shell.

#### Command line arguments

One can pass various command line arguments to the Julia installer. The syntax for installer arguments is

```bash
curl -fsSL https://install.julialang.org | sh -s -- <ARGS>
```

Here `<ARGS>` should be replaced with one or more of the following arguments:
- `--yes` (or `-y`): Run the installer in a non-interactive mode. All configuration values use their default.
- `--default-channel <NAME>`: Configure the default channel. For example `--default-channel lts` would install the `lts` channel and configure it as the default.
- `--path` (or `-p`): Install `juliaup` in a custom location.
    - For example, if you want to install `juliaup` into `~/my/desired/juliaup/path`, you would run the following command: `curl -fsSL https://install.julialang.org | sh -s -- --path ~/my/desired/juliaup/path`

### Software Repositories

**Important note:** As of now, we strongly recommend to install Juliaup via the Windows Store or `curl` command above rather than through OS-specific software repositories (see below) as the Juliaup variants provided by the latter currently have some drawbacks (that we hope to lift in the future).

##### [Homebrew](https://brew.sh)

```
brew install juliaup
```

##### Arch Linux - AUR

On Arch Linux, Juliaup is available in the Arch User Repository (AUR) in two packages.

1. [juliaup](https://aur.archlinux.org/packages/juliaup/) (locally built)
2. [juliaup-bin](https://aur.archlinux.org/packages/juliaup-bin/) (binary from github releases)

##### [openSUSE Tumbleweed](https://get.opensuse.org/tumbleweed/)

On openSUSE Tumbleweed, Juliaup is available. To install, run with root privileges:

```sh
zypper install juliaup
```

##### [Solus](https://getsol.us)

On Solus, Juliaup is available. To install, run with root privileges:

```sh
eopkg install juliaup
```

##### [cargo](https://crates.io/crates/juliaup/)

To install via Rust's cargo, run:

```sh
cargo install juliaup
```

## Continuous Integration (CI)

If you use GitHub Actions as your CI provider, you can use the [`julia-actions/install-juliaup`](https://github.com/julia-actions/install-juliaup) action to install Juliaup in CI.

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
- `juliaup override status` shows all configured directory overrides.
- `juliaup override set lts` sets a directory override for the current working directory to the `lts` channel.
- `juliaup override unset` removes a directory override for the current working directory.
- `juliaup override set --path foo/bar lts` sets a directory override for the path `foo/bar` to the `lts` channel.
- `juliaup override unset --path foo/bar` removes a directory override for the path `foo/bar`.
- `juliaup override unset --nonexistent` removes all directory overrides for paths that no longer exist.
- `juliaup completions bash > ~/.local/share/bash-completion/completions/juliaup` generates Bash completions for `juliaup` and saves them to a file. To use them, simply source this file in your `~/.bashrc`. Other supported shells are `zsh`, `fish`, `elvish` and `powershell`.
- `juliaup` shows you what other commands are available.

The available system provided channels are:
- `release`: always points to the latest stable version.
- `lts`: always points to the latest long term supported version.
- `alpha`: always points to the latest alpha version if one exists. If a newer beta or release candidate exists, it will point to that, and if there is no alpha, beta, or rc candidate available it will point to the same version as the `release` channel.
- `beta`: always points to the latest beta version if one exists. If a newer release candidate exists, it will point to that, and if there is neither a beta or rc candidate available it will point to the same version as the `release` channel.
- `rc`: same as `beta`, but only starts with release candidate versions.
- `nightly`: always points to the latest build from the `master` branch in the Julia repository.
- `x.y-nightly`: always points to the latest build from the `release-x.y` branch in the Julia repository, e.g. `1.11-nightly` gives the latest build on the `release-1.11` branch`.
- `pr{number}` (e.g. `pr123`): points to the latest successful build of a PR branch (https://github.com/JuliaLang/julia/pull/{number}). Only available if CI has recently and successfully built Julia on that branch.
- specific versions, e.g. `1.5.4`.
- minor version channels, e.g. `1.5`.
- major version channels, e.g. `1`.

All of these channels can be combined with the `~x86`, `~x64` or `~aarch64` suffix to download a specific platform version.

## Using installed Julia versions

To launch the default Julia version simply run `julia` in your terminal.

To launch a specific Julia version, say in channel `release`, run `julia +release`.

## Overrides

The Julia launcher `julia` automatically determines which specific version of Julia to launch. There are several ways to control and override which Juliaup channel should be used:

1. A command line Julia version specifier, such as `julia +release`.
2. The `JULIAUP_CHANNEL` environment variable.
3. A directory override, set with the `juliaup override set` command.
3. The default Juliaup channel.

The channel is used in the order listed above, using the first available option.

## Path used by Juliaup

Juliaup will by default use the Julia depot at `~/.julia` to store Julia versions and configuration files. This can be changed by setting
the `JULIAUP_DEPOT_PATH` environment variable. Caution: Previous versions of Juliaup used the content of the environment variable
`JULIA_DEPOT_PATH` to locate Juliaup files, the current version changed this behavior and no longer depends on `JULIA_DEPOT_PATH`.

## Juliaup server

Juliaup by default downloads julia binary tarballs from the official server "https://julialang-s3.julialang.org".
If requested, the environment variable `JULIAUP_SERVER` can be used to tell Juliaup to use a third-party mirror server.

## Development guides

For juliaup developers, information on how to build juliaup locally, update julia versions, and release updates
can be found in the wiki https://github.com/JuliaLang/juliaup/wiki

To use unstable preview versions of juliaup (e.g. to get a patch before it makes it into the latest release), use

```
curl -fsSL https://install.julialang.org/releasepreview | sh
```

## More information

[This JuliaCon 2021 talk](https://www.youtube.com/watch?v=rFlbjWC6zYA) is a short introduction to Juliaup. Note that the video was recorded before the Linux and Mac versions were finished, but all the information about `juliaup` itself applies equally on Linux and Mac.

[This JuliaCon 2022 talk](https://www.youtube.com/watch?v=14zfdbzq5BM) provides some background on the design of Juliaup.
