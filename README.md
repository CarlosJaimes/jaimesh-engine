<!-- Modified by JaiMesh contributors in 2026 from the OpenAI Codex README. -->
# JaiMesh engine

This repository is an independent fork of [OpenAI Codex](https://github.com/openai/codex). It supplies the local app-server and CLI runtime used by the [JaiMesh VS Code extension](https://github.com/CarlosJaimes/JaiMesh). JaiMesh is maintained by Carlos Jaimes and is not affiliated with or endorsed by OpenAI.

We thank OpenAI and the Codex contributors for publishing the engine and app-server protocol as open source. The original project's documentation and installation instructions remain at [openai/codex](https://github.com/openai/codex). Those official installers install OpenAI Codex, not JaiMesh.

## What this fork changes

The JaiMesh branch changes the runtime and package identity, state-directory handling, command behavior and selected prompts used by the extension. Some internal crate names, model identifiers and protocol contracts remain from upstream for compatibility. The extension pins an exact engine revision in [`release/engine-lock.json`](https://github.com/CarlosJaimes/JaiMesh/blob/main/release/engine-lock.json); a new runtime release must update that pin and pass the extension's build and test checks.

For the current Apple Silicon package, follow the [extension development guide](https://github.com/CarlosJaimes/JaiMesh/blob/main/docs/DEVELOPMENT.md). The published source can also be explored with the upstream Rust workspace's `just` tasks. This fork is not a substitute for the official Codex installation instructions.

## License and attribution

The fork remains under the [Apache License 2.0](LICENSE) and retains the upstream [NOTICE](NOTICE). Files changed for JaiMesh carry prominent change notices. The license permits modification and redistribution subject to its conditions; it does not grant rights to OpenAI's trademarks. JaiMesh's separate extension and brand assets have their own [licensing documentation](https://github.com/CarlosJaimes/JaiMesh/blob/main/docs/LICENSING.md).
