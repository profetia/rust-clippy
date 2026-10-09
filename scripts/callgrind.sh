#!/bin/bash

LD_LIBRARY_PATH="$(rustc --print sysroot)/lib" \
valgrind \
    --tool=callgrind \
    --trace-children=yes \
    --callgrind-out-file=callgrind.out.%p \
    "$@"
