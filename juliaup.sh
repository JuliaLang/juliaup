#!/bin/bash
#================================================================
# juliaup bootstrap script
#================================================================
DOWNLOAD_URL="https://github.com/JuliaLang/juliaup/releases/download/v1.2.2/x86_64-unknown-linux-gnu.tar.gz"
DEFAULT_INSTALL_DIR=$HOME/.julia/bin
#TODO: this really needs to be changed
EXTRACTED_DIR='target\x86_64-unknown-linux-gnu'

print_help() {
    cat 1>&2 <<EOF
juliaup-init
The installer for juliaup

USAGE:
    juliaup-init [FLAGS] [OPTIONS]

FLAGS:
    -h, --help            Help information

OPTIONS:
    -d, --install-dir     Specify a directory to store juliaup binary, ~/.julia/juliaup by default.    
EOF
}

check_cmd() {
    command -v "$1" > /dev/null 2>&1
}

downloader() {
    if check_cmd wget; then
        echo wget
    elif check_cmd curl; then
        echo curl
    fi
}

install_juliaup() {
    DLD=`downloader`
    if [[ $DLD =~ wget ]]; then
        wget $DOWNLOAD_URL -O $1
    elif [[ $DLD =~ curl ]]; then
        curl $1 --output $1
    fi
}

cleanup() {
    rm $1/juliaup.tmp.tar.gz
    rm -r $1/$EXTRACTED_DIR
}

main() {
    INSTALLDIR=$DEFAULT_INSTALL_DIR
    while [[ "$1" =~ ^- && ! "$1" == "--" ]]; do case $1 in
        -h | --help )
            print_help
            exit 0
            ;;
        -d | --install-dir )
            shift; INSTALLDIR=$1
    esac; shift; done

    echo "============================= juliaup bootstrap installer ============================"

    if [[ -d $INSTALLDIR ]]; then
        if [[ -e $INSTALLDIR/juliaup ]]; then
            echo "$INSTALLDIR/juliaup already exists"
            exit 1
        fi
    else
        mkdir -p $INSTALLDIR
    fi

    echo "installing to $INSTALLDIR/juliaup..."

    install_juliaup $INSTALLDIR/juliaup.tmp.tar.gz

    tar xzf $INSTALLDIR/juliaup.tmp.tar.gz -C $INSTALLDIR
    mv $INSTALLDIR/$EXTRACTED_DIR/juliaup $INSTALLDIR
    chmod a+x $INSTALLDIR/juliaup

    cleanup $INSTALLDIR

    echo "done."
    exit 0
}

main "$@" || exit 1
