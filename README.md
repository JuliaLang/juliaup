# Juliaup - Julia version manager and Windows Store installer

This repository contains an experimental MSIX installer for Julia for the Windows Store.

The installer also bundles a full Julia version manager called `juliaup`. One can use `juliaup` to install specific Julia versions, it alerts users when new Julia versions are released and provides a convenient Julia release channel abstraction. The installer is published to the Windows Store and you can try it from [here](https://www.microsoft.com/store/apps/9NJNWW8PVKMN).

## Using Juliaup

If you want to try it, here is what you should do:

### Windows Users
Make sure you don't have any version of Julia on your PATH. `Juliaup` will handle all `PATH` related aspects of your Julia installation.

#### Option 1: The Microsoft Store
- Install Julia from the Windows Store [here](https://www.microsoft.com/store/apps/9NJNWW8PVKMN).

Once you have that installed, `julia` is on the `PATH`, there is a start menu shortcut and it will show up as a profile in Windows Terminal. Any of those will start Julia. The VS Code extension will also automatically find this Julia installation.

#### Option 2: User Level Installation Script
- Run `powershell -nop -c iex (New-Object System.Net.WebClient).DownloadString('https://raw.github.com/Julialang/juliaup/master/juliaup-init.sh.cmd')` 
    or [download and run](https://raw.github.com/Julialang/juliaup/master/juliaup-init.sh.cmd) and follow the prompt.
- Note, it's preferable to choose `yes` to _add juliaup.exe and julia.exe to user PATH_. This will allow the VS Code extension and other programs to access `julia`.

### Linux and macOS Users
- Run `curl https://raw.github.com/Julialang/juliaup/master/juliaup-init.sh.cmd | sh` or [download and run](https://raw.githubusercontent.com/Julialang/juliaup/master/juliaup-init.sh.cmd) to install juliaup.
- Optionally, add the `~/.juliaup/bin` binaries to your `PATH`. Preferably, add symlinks via sudo privileges: `sudo ln "$HOME/.juliaup/bin/juliaup" /usr/local/bin/juliaup -s; sudo ln "$HOME/.juliaup/bin/julialauncher" /usr/local/bin/julia -s`.
    Alternatively, add the line `export PATH=$HOME/.juliaup/bin:$PATH` to `~/.profile` (see [this explanation](https://unix.stackexchange.com/a/26059)).
- Run `juliaup` with the commands described below.

### Subcommands
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

The available system provided channels are:
- `release`: always points to the latest stable version.
- `lts`: always points to the latest long term supported version.
- `beta`: always points to the latest beta version if one exists. If a newer release candidate exists, it will point to that, and if there is neither a beta or rc candidate available it will point to the same version as the `release` channel.
- `rc`: same as `beta`, but only starts with release candidate versions.
- specific versions, e.g. `1.5.4`.
- minor version channels, e.g. `1.5`.
- major version channels, e.g. `1`.

All of these channels can be combined with the `~x86` or `~x64` suffix to download a specific platform version.

This entire system around `juliaup` installs Julia versions into `~/.julia/juliaup`. If you want to restart from scratch, just delete that entire folder.
