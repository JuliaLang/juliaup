# juliaup PATH and completions
if not contains {bin_path} $PATH
    set -x PATH {bin_path} $PATH
end
if test -f "{juliauphome}/completions/fish.fish"
    source "{juliauphome}/completions/fish.fish"
end
