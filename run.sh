#!/bin/bash

# Load environment variables from .env file
set -o allexport
source .env
set +o allexport

# Build binary if requested
if [ "$#" -eq 1 ]; then
    if [ "$1" = "-b" ]; then
        echo "Building binary..."
        cargo build --release
    fi
fi

./target/release/app
