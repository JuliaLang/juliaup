case ":$PATH:" in
    *:{bin_path}:*)
        ;;

    *)
        export PATH={bin_path}${PATH:+:${PATH}}
        ;;
esac
