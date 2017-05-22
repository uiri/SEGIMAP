#!/bin/sh

export PATH=$HOME/.local/bin/:$PATH

travis-cargo build
travis-cargo test
travis-cargo bench
travis-cargo --only stable doc
