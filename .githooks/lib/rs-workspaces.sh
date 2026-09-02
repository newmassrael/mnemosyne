#!/usr/bin/env bash
# .githooks/lib/rs-workspaces.sh — which workspaces a set of `.rs` paths live in.
#
# WHY THIS EXISTS. A gate's TRIGGER and its TARGET have to be the same tree, and
# in this repository they were not. Five gates across the two hooks fired on
# "some `.rs` changed" and then walked the ROOT workspace, while the repository
# has TWENTY-SIX manifests carrying their own `[workspace]` — so a change under
# any of the other twenty-five was graded by a tree that does not contain it.
# Four whole-workspace walks and a format check would run, report findings about
# files the change never touched, and say nothing about the file it did. Zero
# findings is what a clean tree looks like.
#
# MEASURED BY IT HAPPENING. R1291 added a test under `tools/injection-harness/`
# that spawns a binary reading `CARGO` without naming it. `named-environment` is
# the gate for exactly that, it RAN on that commit, over the root, and passed;
# the hosted `separate in-repo workspaces` job found it one round later. R1292
# repaired the four `.rs` tool gates in `pre-commit` and left the format check —
# in BOTH hooks — still pointed at the root, which is this file's other half.
#
# ONE DEFINITION AND NOT TWO, which is why it is a library rather than a block in
# each hook. `pre-commit` asks it about the STAGED list and `pre-push` about the
# PUSHED RANGE, and those are different questions with the same answer shape;
# two spellings of "which workspace owns this file" is a pair that can disagree
# silently, and the disagreement would read as a gate that found nothing. It is
# the same reason `ident-gate.sh` is sourced by both hooks rather than pasted.
#
# THE WALK IS LEXICAL, so a path the working tree no longer has still resolves.
# `pre-push` grades a RANGE, and a file added in one commit and deleted in
# another is in it; entering its directory would fail where naming its manifest
# does not. A directory that is gone simply has no `Cargo.toml`, and the walk
# continues upward, which is the same answer a person would give.

# Resolve the workspaces owning every `.rs` in a newline-separated path list.
#
# Sets RS_WORKSPACES to the absolute manifest paths, in first-seen order, and
# returns 0. On anything it cannot answer it prints why, prefixed with the
# caller's label, and returns 1 — a file this walk could not place is a file the
# gates above would grade against the wrong tree, which is the state it exists to
# end.
rs_workspaces_of() {
    local label="$1" tree_root="$2" paths="$3"
    local path dir manifest
    RS_WORKSPACES=()
    while IFS= read -r path; do
        [[ "$path" == *.rs ]] || continue
        # LEXICAL, FROM THE TREE ROOT: git spells its paths relative to it, and
        # this must answer for paths whose directory no longer exists.
        dir="$tree_root/${path%/*}"
        [[ "$path" == */* ]] || dir="$tree_root"
        while :; do
            manifest="$dir/Cargo.toml"
            if [[ -f "$manifest" ]]; then
                # A MANIFEST THAT EXISTS AND CANNOT BE READ IS NOT ONE WITHOUT A
                # `[workspace]`. Walking past it hands this file to whatever
                # workspace lies further up, which is the mispointing above with
                # an extra step in front of it.
                if [[ ! -r "$manifest" ]]; then
                    {
                        echo "$label: $manifest exists and cannot be read, so which"
                        echo "  workspace grades $path is unanswerable"
                    } >&2
                    return 1
                fi
                if grep -q '^\[workspace\]' "$manifest"; then
                    case " ${RS_WORKSPACES[*]-} " in
                    *" $manifest "*) ;;
                    *) RS_WORKSPACES+=("$manifest") ;;
                    esac
                    break
                fi
            fi
            if [[ "$dir" == "$tree_root" || "$dir" == "/" ]]; then
                {
                    echo "$label: no [workspace] above $path up to $tree_root, so the"
                    echo "  gates keyed on it would read a tree this file is not in"
                } >&2
                return 1
            fi
            dir="${dir%/*}"
        done
    done <<< "$paths"
    return 0
}
