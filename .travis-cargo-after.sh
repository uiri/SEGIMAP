#!/bin/sh

export PATH=$HOME/.local/bin/:$PATH

travis-cargo --only stable doc-upload
travis-cargo coveralls --no-sudo --verify
