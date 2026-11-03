#!/usr/bin/env bash
#let _ = ```sh
case "$1" in
  compile) typst compile --features bundle,html --format bundle $0 ./dist/docs; exit 0 ;;
  watch)   typst watch   --features bundle,html --format bundle --pretty $0 ./dist/docs; exit 0 ;;
  *)       echo "Unknown option: $1. Enter 'compile' or 'watch'"; exit 1 ;;
esac
exit 1
```

#include "docs/book.typ"
