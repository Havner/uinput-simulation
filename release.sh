#!/bin/bash

PLUGIN="com.havner.toolkit.sdPlugin"
TARGET="x86_64-unknown-linux-gnu"

rm -rf "$PLUGIN"                                                       || exit 1
mkdir "$PLUGIN"                                                        || exit 1
cp -r assets/* "$PLUGIN"/                                              || exit 1
cargo install --target "$TARGET" --path . --root "$PLUGIN/$TARGET"     || exit 1
