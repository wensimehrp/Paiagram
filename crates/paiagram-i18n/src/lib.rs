// SPDX-License-Identifier: MPL-2.0
use fluent::{FluentBundle, FluentResource};

fn make_fluent() {
    let res = FluentResource::try_new("foo bar".into());
}
