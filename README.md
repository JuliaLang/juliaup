# Juliaup - Julia version manager and Windows Store installer

This repository contains an experimental MSIX installer for Julia for the Windows Store.

The installer also bundles a full Julia version manager called `juliaup`. One can use `juliaup` to install specific Julia versions, it alerts users when new Julia versions are released and provides a convenient Julia release channel abstraction. The installer is published to the Windows Store and you can try it from [here](https://www.microsoft.com/store/apps/9NJNWW8PVKMN).

## Using Juliaup

If you want to try it, here is what you should do:
- Make sure you don't have any version of Julia on your PATH. `Juliaup` will handle all `PATH` related aspects of your Julia installation.
- Install Julia from the Windows Store [here](https://www.microsoft.com/store/apps/9NJNWW8PVKMN).

Once you have that installed, `julia` is on the `PATH`, there is a start menu shortcut and it will show up as a profile in Windows Terminal. Any of those will start Julia. The VS Code extension will also automatically find this Julia installation.

Here are some of the things you can do with `juliaup`:
- `juliaup update` installs the latest availabe Julia version for your current channel.
- `juliaup status` shows you which Julia versions you have installed and which one is configured as the default.
- `juliaup add 1.5.1` adds Julia 1.5.1 to your system (it can then be launched via the command `julia +1.5.1`).
- `juliaup setdefault 1.5.3` configures the `julia` command to start Julia 1.5.3.
- `juliaup setdefault 1.6` configures the `julia` command to start the latest 1.6.x version of Julia you have installed on your system (and inform you if there is a newer version in 1.6.x available).
- `juliaup setdefault 1` configures the `julia` command to start the latest 1.x version of Julia (this is also the default value).
- `juliaup remove 1.5.3` deletes Julia 1.5.3 from your system.
- `juliaup add 1.6.1~x86` installs the 32 bit version of Julia 1.6.1 on your system.
- `juliaup setdefault 1.6~x86` configures the `julia` command to start the latest 1.6.x 32 bit version of Julia you have installed on your system.
- `juliaup` shows you what other commands are available.

This entire system around `juliaup` installs Julia versions into `~/.julia/juliaup`. If you want to restart from scratch, just delete that entire folder.
