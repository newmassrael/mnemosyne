//! Emit `BUILD_GIT_HASH` so `mnemosyne-cli --version` can say which revision
//! produced this binary. The rule itself lives in `mnemosyne-build-stamp` —
//! one home, because this file and `mnemosyne-mcp`'s were transcriptions of
//! each other and carried the same defect from Round 286 to Round 823.

fn main() {
    mnemosyne_build_stamp::emit();
}
