# Paiagram

UI components for Paiagram.

This crate uses `egui`, which should run in the browser, Linux, macOS, and Windows.

## World Snapshot

```
start of frame
╭─────────╮
│^    ╭───┴───╮
│^  world   clone
│^    │       │
│     ├─ read ├─ mutate
│     ├─ read ├─ mutate
│     ├─ read ├─ mutate
│     │       │
│  compare diff and update cache
│     ╰───┬───╯
│  updated version
╰─────────╯
end of frame
```
