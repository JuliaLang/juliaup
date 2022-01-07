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

### Mac and Linux

A prerelease version of the installer can be installed on Linux or Mac by executing

```
curl -fsSL https://install.julialang.org/releasepreview | sh
```

in a shell. Note that the Mac and Linux version are considered prerelease, have known bugs and might often break.

### Arch Linux

On Arch Linux this installer and Juliaup are available in the system package manager.

## Using Juliaup

Once you have installed Juliaup, `julia` is on the `PATH`, and on Windows there is a start menu shortcut and it will show up as a profile in Windows Terminal. Any of those will start Julia. The VS Code extension will also automatically find this Julia installation.

Here are some of the things you can do with `juliaup`:
- `juliaup update` installs the latest availabe Julia version for all your channels.
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
- `juliaup` shows you what other commands are available.

To launch the Julia version in channel `release`, run `julia +release` in your terminal.

The available system provided channels are:
- `release`: always points to the latest stable version.
- `lts`: always points to the latest long term supported version.
- `beta`: always points to the latest beta version if one exists. If a newer release candidate exists, it will point to that, and if there is neither a beta or rc candidate available it will point to the same version as the `release` channel.
- `rc`: same as `beta`, but only starts with release candidate versions.
- specific versions, e.g. `1.5.4`.
- minor version channels, e.g. `1.5`.
- major version channels, e.g. `1`.

All of these channels can be combined with the `~x86`, `~x64` or `~aarch64` suffix to download a specific platform version.
