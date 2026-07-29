//! Emit `BUILD_GIT_HASH` so `mnemosyne-mcp --version` can say which revision
//! produced this binary. The rule lives in `mnemosyne-build-stamp`; see the
//! `mnemosyne-cli` build script for why it is not written twice any more.

fn main() {
    mnemosyne_build_stamp::emit();
}
