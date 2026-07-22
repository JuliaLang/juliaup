case ":$PATH:" in
    *:{bin_path}:*)
        ;;

    *)
        export PATH={bin_path}${PATH:+:${PATH}}
        ;;
esac
# Tab completion for juliaup and julia channel selection
[ -f "{juliauphome}/completions/bash.sh" ] && source "{juliauphome}/completions/bash.sh"
