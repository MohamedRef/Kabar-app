#!/bin/bash
# vcheck.sh [repo]: run only the content-validity test and print its problems
cd ${1:-.}
touch crates/content/tests/*.rs crates/content/src/*.rs
RUST_BACKTRACE=0 cargo test -q -p kabar-content --test content all_content_is_valid 2>&1 | grep -E "^(bipa|j[0-9]|id/|jv/)|yaml|test result|problem" | sed 's|.*content/lessons/||'
