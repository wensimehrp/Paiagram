# Paiagram User Docs

This is the user documentation of Paiagram. It is intended for Paiagram users, and not for
developers. If you are a developer, please see refer to the result of `cargo doc` instead!

# Authoring

The documentation uses [Typst](https://typst.app) and
[Haita](https://wensimehrp.github.io/haita).

You can build the documentation and the site by running `./site.typ compile` in the
**workspace root** (i.e. Paiagram/). This produces the website _and_ the user documentation in a
folder named `site`. You can also launch a local development server by running `./site.typ watch`.
There is no way to only build the docs without building the site. The design is intentional.
