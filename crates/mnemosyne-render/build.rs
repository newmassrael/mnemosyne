//! Emit `BUILD_GIT_HASH` so this binary can identify itself to the tool pin.
//! The rule lives in `mnemosyne-build-stamp` (Round 824).

fn main() {
    mnemosyne_build_stamp::emit();
}
