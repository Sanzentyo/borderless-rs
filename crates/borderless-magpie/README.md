<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# borderless-magpie

`borderless-magpie` contains the Magpie-compatible scaler port for Borderless Oxide.

This crate is licensed under GPL-3.0-or-later. Code in this crate that directly follows Magpie
behavior, file formats, render planning, shader semantics, or effect asset behavior inherits that GPL
boundary.

Reference project:

- Magpie: <https://github.com/Blinue/Magpie>

Permissive crates such as `borderless-core` and `borderless-upscale-core` must not depend on this
crate unless the resulting build is intentionally GPL.
