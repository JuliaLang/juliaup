# Julia MSIX installer

NOTE: The version currently published in the Windows Store ships an outdated Julia version and the `juliaup` utility is not functional.

This repository contains an experimental MSIX installer for Julia for the Windows Store.

The installer also bundles a full Julia version manager called `juliaup`. One can use `juliaup` to install specific Julia versions, it alerts users when new Julia versions are released and provides a convenient Julia release channel abstraction.

The installer is published to the Windows Store and you can try it from [here](https://www.microsoft.com/store/apps/9NJNWW8PVKMN). BUT BE WARNED: I am actively working on this installer, I am breaking things frequently and at this point it is probably inadvisable to use this, even if you consider yourself an early-adopter!

If you do want to try it out, things are simple: just go to the Windows Store page for the Julia installer [here](https://www.microsoft.com/store/apps/9NJNWW8PVKMN) and install the app from there. That will install the latest version of Julia on your system and also automatically add it to the `PATH` (even into a running shell, no need to restart a shell you had open _before_ you installed Julia!). You can then explore `juliaup` by simply running the `juliaup` command from your favorite shell (also automatically on the `PATH`). Valid version version identifiers you can use with `juliaup` are things like this: `1.6.1`, `1.6`, `1`, `1.61-x86`, `1.6-x86` etc.
