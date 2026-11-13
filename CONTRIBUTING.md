# Contributor's Notes

Anyone is welcomed to contribute to this project, as long as the contributions are helpful.

## Components

This Repo (wensimehrp/Paiagram) is the monorepo for Paiagram-related projects, including:

- Paiagram Web (`crates/**`)
- Paiagram Desktop (`crates/**`)
- paiagram.com (`site.typ`)
- User documentation for Paiagram (hosted on paiagram.com, `docs.typ` and `docs/`)
- Translations for Paiagram-related components

Most directories have a `README.md` file with them, which describes the contents of the directory.

## Translating

You can contribute translations by editing the translation files in the `paiagram` crate.
The translations are [Fluent translation files](https://projectfluent.org), which should be easy to
edit and flexible. Please don't submit any unreviewed machine/LLM translations. I absolutely hate
them.

## Compiling

- For Nix users: simply run `nix develop .#default` and `cargo run --release`
- For non-Nix users: you'll need the dependencies listed in [`flake.nix`](./flake.nix).
  Specifically, look for `runtimeLibs` and `buildInputs`.

## AI Policy

We don't encourage or discourage the use of AI/LLM, though such tools are already used in the
development of this app. In general, you may use it to do the following:

- Improve your work.
- Write documentation.

You may not use it to do the following:

- Translate any components to Simplified Chinese.
- Translate any components to Traditional Chinese,
  unless using Simplified Chinese as input (i.e. 简繁转换).
- Write PR/issue description.
- Submit PR/issue/discussion.

Any AI-generated description will be removed. AI generated Issues, PRs, and Discussions
will be closed.

AI/LLM is a useful tool; however it must be used by the right person in the right way.
AI submission without human guidance only wastes the maintainer's time. Each submission
should teach both the maintainer and the submitter something new, but if the submitter
doesn't even know what changes are made, it is hard to tell if the submission is truly
helpful. You should not submit anything if you don't even understand what the AI is doing.
