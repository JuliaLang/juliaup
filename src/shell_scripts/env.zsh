path=('{bin_path}' $path)
export PATH
# Tab completion for juliaup and julia channel selection
[ -f "{juliauphome}/completions/zsh.zsh" ] && source "{juliauphome}/completions/zsh.zsh"
