#!/bin/sh
# Checkout builds: every port's .horizon/sdk/api is a link to one workspace over this
# repository's crates, so `cargo check` in a port uses the current mod API (and a port that
# depends on another, such as justenoughbreeding on jei, sees one copy of it). The launcher
# fetches the real SDK itself.
cd "$(dirname "$0")" || exit 1
root=$(cd ../.. && pwd)
shared="$(pwd)/.dev-sdk/api"
rm -rf "$shared"; mkdir -p "$shared"
ln -s "$root/crates" "$shared/crates"
cat > "$shared/Cargo.toml" <<'TOML'
[workspace]
members = ["crates/horizon-mod-api"]
resolver = "3"

[workspace.package]
edition = "2024"
license = "LGPL-3.0-or-later"
rust-version = "1.95"

[workspace.lints.clippy]
too_many_arguments = "allow"
type_complexity = "allow"
needless_range_loop = "allow"
large_enum_variant = "allow"
TOML
for d in */; do
  [ -f "$d/horizon-mod.toml" ] && [ -f "$d/Cargo.toml" ] || continue
  rm -rf "$d/.horizon"; mkdir -p "$d/.horizon/sdk"
  ln -s "$shared" "$d/.horizon/sdk/api"
done
