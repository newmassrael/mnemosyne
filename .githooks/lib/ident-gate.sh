#!/usr/bin/env bash
# .githooks/lib/ident-gate.sh — which addresses may author or commit here.
#
# WHY THIS EXISTS, and it is not hypothetical. In a sibling repository of this
# owner's, eight commits reached a PUBLIC remote authored with a work address
# instead of the one every other commit carries. A history rewrite removed
# them from the branch and that did NOT un-publish them: the host kept the
# unreachable objects, went on serving all eight by SHA for days, and went on
# listing the other account as a contributor the whole time. What actually
# removed them was deleting and recreating the repository, which cost 1146
# runs of CI history. This gate exists so the next occurrence costs a refused
# commit instead.
#
# WHY A CONFIG WOULD NOT HAVE CAUGHT IT. `git config user.email` was already
# correct there, in the clone AND in ~/.gitconfig. The commits came from a
# different environment. A config is a DEFAULT and it is per-clone; a rule
# that has to be present on the machine that gets it wrong has to travel with
# the tree, which is what a tracked hook does.
#
# WHY IT GRADES `git var` AND NEVER `git config`. The identity a commit will
# carry is not what the config says: GIT_AUTHOR_EMAIL and GIT_COMMITTER_EMAIL
# in the environment override it, `commit --author` overrides it again, and
# `git -c user.email=` overrides it for one invocation. `git var
# GIT_AUTHOR_IDENT` is git answering with what it will actually stamp, after
# all of that. Reading the config grades a different question -- the shape
# this repository names elsewhere as a gate keyed on something other than what
# it claims to measure.
#
# WHY AN ALLOW-LIST. A deny-list passes every identity it has not been taught
# yet; an allow-list fails closed for the one nobody thought about. It also
# avoids writing the offending address into a tracked file of a public
# repository, which is the exposure the incident was about.
#
# WHY A LIBRARY IN A TREE THAT HAS NO OTHER ONE. Both hooks need the same
# list, and a list written twice is two facts that can disagree -- which is
# the defect this repository exists to make impossible, so it should not be
# introduced in its own hooks. One file, sourced twice.
#
# Gated by `crates/mnemosyne-cli/tests/git_hooks_smoke.rs`, which holds an
# accept/reject pair for every gate the hooks run.

# The identities this repository accepts. Add one DELIBERATELY, in its own
# commit: an edit here is a statement about who may write history that the
# remote publishes.
#
# The single entry is measured, not assumed: every commit in this tree is
# authored and committed by it, with one exception that is NOT here on
# purpose -- a harness artifact written by a tool when it hit a rate limit.
# Allowing it would license the next one.
MNEMOSYNE_ALLOWED_IDENT_EMAILS=(
    "newmassrael@gmail.com"
)

# "Name <email> 1756100000 +0900" -> "email".
#
# Cut on the angle brackets, not on whitespace: a display name may contain
# spaces, and a field-counting parse returns the wrong token when it does --
# silently, which is the failure mode this file is about.
ident_email_of() {
    local ident="$1"
    ident="${ident#*<}"
    printf '%s' "${ident%%>*}"
}

ident_is_allowed() {
    local email="$1" allowed
    for allowed in "${MNEMOSYNE_ALLOWED_IDENT_EMAILS[@]}"; do
        [[ "$email" == "$allowed" ]] && return 0
    done
    return 1
}

# The shared refusal, so the two hooks cannot drift into explaining one rule
# two ways.
ident_refuse() {
    local hook="$1" what="$2" email="$3"
    {
        echo "[mnemosyne ${hook}] ${what} <${email}>,"
        echo "  which is not an identity this repository accepts."
        echo ""
        echo "  Measured: eight commits reached a PUBLIC repo of this owner's"
        echo "  under a different address. Rewriting history did NOT un-publish"
        echo "  them -- they stayed reachable by SHA and the repository had to"
        echo "  be deleted and recreated, costing 1146 runs of CI history."
        echo ""
        echo "  fix, in this clone:"
        echo "    git config user.email ${MNEMOSYNE_ALLOWED_IDENT_EMAILS[0]}"
        echo "    git config user.name  <your name>"
        echo "  and check the environment too -- these override the config:"
        echo "    env | grep -E '^GIT_(AUTHOR|COMMITTER)_EMAIL='"
        echo ""
        echo "  If a NEW identity is genuinely meant to write here, add it to"
        echo "  MNEMOSYNE_ALLOWED_IDENT_EMAILS in .githooks/lib/ident-gate.sh"
        echo "  -- deliberately, in its own commit."
    } >&2
}

# pre-commit's arm: the identity the commit ABOUT TO BE MADE would carry.
ident_gate_pending() {
    local hook="$1" pair role verb ident email
    for pair in "AUTHOR authored" "COMMITTER committed"; do
        role="${pair%% *}"
        verb="${pair##* }"
        if ! ident="$(git var "GIT_${role}_IDENT")"; then
            echo "[mnemosyne ${hook}] \`git var GIT_${role}_IDENT\` failed" >&2
            echo "  cannot determine the identity this commit would carry, and" >&2
            echo "  a gate that could not read is not a gate that found nothing" >&2
            return 1
        fi
        email="$(ident_email_of "$ident")"
        if [[ -z "$email" ]]; then
            echo "[mnemosyne ${hook}] no email in GIT_${role}_IDENT: ${ident}" >&2
            return 1
        fi
        if ! ident_is_allowed "$email"; then
            ident_refuse "$hook" "this commit would be ${verb} as" "$email"
            return 1
        fi
    done
    return 0
}

# pre-push's arm: every commit in the range being published.
#
# `range` is `<base>..<tip>` when the remote already has the ref and a bare
# `<tip>` when it does not. That second case grades the WHOLE history and is
# deliberate: a brand-new remote is exactly when a stray identity would
# otherwise be republished wholesale.
#
# Reports EVERY offender rather than the first, because the fix is a rebase
# whose scope the author needs to know before starting it.
ident_gate_range() {
    local hook="$1" range="$2" sha email bad=0 shown=0 log
    if ! log="$(git log --format='%H %ae%n%H %ce' "$range" --)"; then
        echo "[mnemosyne ${hook}] \`git log ${range}\` failed" >&2
        echo "  cannot determine which identities this push would publish, and" >&2
        echo "  a gate that could not read is not a gate that found nothing" >&2
        return 1
    fi
    while IFS=' ' read -r sha email; do
        [[ -n "$sha" ]] || continue
        if ! ident_is_allowed "$email"; then
            if [[ "$bad" -eq 0 ]]; then
                ident_refuse "$hook" "this push would publish commits by" "$email"
                echo "" >&2
                echo "  offending commits in ${range}:" >&2
            fi
            bad=$((bad + 1))
            if [[ "$shown" -lt 20 ]]; then
                echo "    ${sha} <${email}>" >&2
                shown=$((shown + 1))
            fi
        fi
    done <<<"$log"
    if [[ "$bad" -gt 0 ]]; then
        [[ "$bad" -gt "$shown" ]] && echo "    ... and $((bad - shown)) more" >&2
        return 1
    fi
    return 0
}
