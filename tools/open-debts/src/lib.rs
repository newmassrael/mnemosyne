//! What the outstanding-debt ledger says is still this session's to do.
//!
//! THE LEDGER IS PROSE WITH A NOTATION, and the notation is the only thing this
//! reads. A debt is registered as `**<id>**(<classification>)` — as a bullet
//! whose body follows it, or inline inside a round summary where the
//! parenthetical IS the body — and retired either by the word `CLOSED` in its
//! own row or by being struck through, `~~<id>~~`, which is what the ledger's
//! tables use.
//!
//! EVERY RULE HERE WAS PAID FOR BY THE CENSUS BEING WRONG. See `Cargo.toml` for
//! the three, and note what they have in common: each made the count SMALLER
//! than the work or LARGER than the work in a way nothing announced. A census
//! that can be wrong quietly is the same defect as a gate that reports zero
//! findings over a tree it never read.

use std::collections::{BTreeMap, BTreeSet};

/// The letters a debt id may start with, as the ledger has used them.
const PREFIXES: [char; 8] = ['N', 'Z', 'Y', 'A', 'T', 'W', 'S', 'B'];

/// The classification markers the ledger writes, circled digits one to five.
///
/// ONE IS THE BRANCH THIS CENSUS IS ABOUT and the rest are here so that a row
/// carrying `①/③` — a genuine "either of these" — is counted once and its
/// ambiguity is visible rather than silently resolved.
pub const AUTONOMOUS: char = '\u{2460}';

/// Whether a character is one of the ledger's five branch markers.
fn a_branch_marker(c: char) -> bool {
    ('\u{2460}'..='\u{2464}').contains(&c)
}

/// How a registration was written, which decides where its BODY is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// `- **N224**(①) — the body runs to the next bullet or blank line.`
    Bullet,
    /// `… · **N216**(the body is the parenthetical, ①) · …`
    Inline,
}

/// One place the ledger registers a debt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    pub id: String,
    /// 1-based, so a reader can open the file at it.
    pub line: usize,
    pub shape: Shape,
    /// The text this registration is judged on: the row for a bullet, the
    /// parenthetical for an inline one.
    pub body: String,
}

impl Registration {
    /// Whether this is somebody NAMING a debt rather than registering one.
    ///
    /// `신규 = **N123**(①자율)` in a round summary is a sentence about a debt
    /// that lives elsewhere; the parenthetical holds a marker and the word for
    /// the branch and nothing to do. Counting it opens a row that can never be
    /// closed, because there is nothing there to close — which is exactly the
    /// shape that makes a termination condition unreachable.
    ///
    /// JUDGED ON WHAT IS LEFT AFTER THE CLASSIFICATION, so no length threshold
    /// decides it: strip the branch markers and the words the ledger writes
    /// beside them, and a mention has nothing remaining.
    #[must_use]
    pub fn is_a_mention(&self) -> bool {
        if self.shape != Shape::Inline {
            return false;
        }
        // THE PUNCTUATION GOES IN THE SAME PASS AS THE MARKERS, because it is the
        // same kind of thing: `①/③` and `②→①?` are one classification written with
        // separators, and dropping the separators one call at a time was five
        // chained `replace`s that clippy is right to call one.
        let left: String = self
            .body
            .chars()
            .filter(|c| !a_branch_marker(*c) && !c.is_whitespace() && !"/?,→".contains(*c))
            .collect();
        // AND THE ONE WORD THE LEDGER WRITES BESIDE A MARKER. `자율` is what the
        // branch is called in prose, so `(①자율)` is a classification and nothing
        // else — which is the whole of what makes it a mention rather than a row.
        left.replace("자율", "").is_empty()
    }

    /// Whether this registration carries the autonomous marker.
    #[must_use]
    pub fn is_autonomous(&self) -> bool {
        match self.shape {
            Shape::Inline => self.body.contains(AUTONOMOUS),
            // A BULLET'S MARKER IS ITS PARENTHETICAL AND NOT ITS WHOLE ROW: the
            // body of a row may cite another branch in prose ("this belongs with
            // the ② rows"), and reading that as a classification would file the
            // row under every branch it mentions.
            Shape::Bullet => parenthetical_of(&self.body, &self.id)
                .is_some_and(|marked| marked.contains(AUTONOMOUS)),
        }
    }
}

/// The parenthetical immediately after `**<id>**` in a piece of text.
fn parenthetical_of(text: &str, id: &str) -> Option<String> {
    let needle = format!("**{id}**(");
    let at = text.find(&needle)? + needle.len();
    parenthetical_at(text, at)
}

/// The parenthetical that OPENS one byte before `at`, balanced.
fn parenthetical_at(text: &str, at: usize) -> Option<String> {
    if at > text.len() {
        return None;
    }
    let mut depth = 1usize;
    let mut end = at;
    for (offset, c) in text[at..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = at + offset;
                    break;
                }
            }
            _ => {}
        }
    }
    Some(text[at..end].to_string())
}

/// Whether a byte sequence at this position spells a debt id, and how long.
fn id_at(bytes: &[u8], at: usize) -> Option<usize> {
    let first = *bytes.get(at)? as char;
    if !PREFIXES.contains(&first) {
        return None;
    }
    let mut end = at + 1;
    let mut digits = 0;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
        digits += 1;
    }
    if digits == 0 {
        return None;
    }
    // A SUFFIX LIKE `-old` IS PART OF THE ID, because the ledger uses it to keep
    // a superseded row beside its replacement and the two are different rows.
    while end < bytes.len()
        && (bytes[end] == b'-' || bytes[end].is_ascii_lowercase() || bytes[end].is_ascii_digit())
    {
        end += 1;
    }
    Some(end - at)
}

/// Every registration in a ledger, in the order they appear.
/// THE SCAN IS OVER THE WHOLE TEXT AND NOT LINE BY LINE, because an inline
/// parenthetical WRAPS — the ledger writes long ones across two lines and the
/// branch marker is usually on the second. A line-at-a-time reader sees a
/// parenthetical that never closes, reads a body with no marker in it, and drops
/// the row: four real rows, silently, which is how this crate's first census
/// came back five lower than the one-liner it replaces.
#[must_use]
pub fn registrations(ledger: &str) -> Vec<Registration> {
    let lines: Vec<&str> = ledger.lines().collect();
    // Byte offset of the start of each line, so a match in the flat text can say
    // which line a reader should open.
    let mut starts = Vec::with_capacity(lines.len());
    let mut running = 0usize;
    for line in &lines {
        starts.push(running);
        running += line.len() + 1;
    }
    let line_of = |offset: usize| -> usize {
        match starts.binary_search(&offset) {
            Ok(exact) => exact,
            Err(after) => after.saturating_sub(1),
        }
    };
    let bytes = ledger.as_bytes();
    let mut found = Vec::new();
    let mut at = 0usize;
    {
        while let Some(star) = ledger[at..].find("**") {
            let start = at + star + 2;
            at = start;
            let Some(width) = id_at(bytes, start) else {
                continue;
            };
            let id = &ledger[start..start + width];
            if !ledger[start + width..].starts_with("**(") {
                continue;
            }
            let index = line_of(start);
            let line = lines[index];
            let bullet = line.trim_start().starts_with(&format!("- **{id}**"));
            let (shape, body) = if bullet {
                // A BULLET'S ROW IS ITS OWN LINE PLUS ITS INDENTED CONTINUATIONS,
                // which is the notation the ledger actually uses: a wrapped row
                // begins with a space, and anything starting at column zero is a
                // new paragraph. Getting this boundary wrong is not theoretical
                // and it has been wrong TWICE — read to the next registration
                // anywhere, a row swallowed the `CLOSED` of the bullet beneath it
                // and retired three live rows; read to the next blank line, it
                // swallowed the prose paragraph beneath it and retired one more.
                let mut end = index + 1;
                while end < lines.len()
                    && !lines[end].trim().is_empty()
                    && lines[end].starts_with(char::is_whitespace)
                {
                    end += 1;
                }
                (Shape::Bullet, lines[index..end].join("\n"))
            } else {
                // FROM THE FLAT TEXT so a wrapped parenthetical is one
                // parenthetical, and from THIS registration's own offset so two
                // ids on one line each get their own.
                (
                    Shape::Inline,
                    // `start + width` ends the id; `**(` is three bytes more, and
                    // the content begins after the paren.
                    parenthetical_at(ledger, start + width + 3).unwrap_or_default(),
                )
            };
            found.push(Registration {
                id: id.to_string(),
                line: index + 1,
                shape,
                body,
            });
        }
    }
    found
}

/// The word this ledger retires a row with.
///
/// NAMED ONCE BECAUSE TWO READERS OF IT DISAGREED (R1298). `retired` demanded a
/// run of ids reaching the word; `open_autonomous` accepted it anywhere in a
/// row. The same six letters meant two different things one function apart —
/// the two-write-paths-one-invariant shape this project's own `CLAUDE.md`
/// forbids, and the looser path is the one that decided the count.
const RETIREMENT: &str = "CLOSED";

/// What a retirement names as the thing that retired the row.
///
/// A RETIREMENT THAT NAMES NOTHING IS NOT ONE (R1298). Every genuine retirement
/// this ledger has written says WHO: a round, a commit, or the owner's word. A
/// closure nobody can chase is a sentence, and this repository's whole doctrine
/// is that a verdict naming nothing has no reader.
///
/// AND REQUIRING THE NAME IS WHAT LETS A ROW BE *ABOUT* RETIREMENT. N270 was
/// registered, said the word in its own headline, and retired itself in the same
/// breath — the census reported 26 open with the row simply absent, which is a
/// green that means nobody looked.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Attribution {
    /// `R1297` / `Round 1297`.
    pub round: Option<String>,
    /// The sha spelled after the word `커밋` / `commit`.
    pub commit: Option<String>,
    /// `CLOSED 2026-08-14` — the day, which is chaseable in the log.
    ///
    /// FOUND BY BEING WRONG, and kept for that reason. The first form of this
    /// rule knew three names and refused two live retirements on the real
    /// ledger; one of them, `Z19`, is attributed by a DATE and by nothing else.
    /// The ledger's notation is the evidence here and this reader is not, so
    /// the rule widened rather than the ledger being rewritten to suit it.
    pub date: Option<String>,
    /// The owner said so, which is this ledger's fourth branch.
    pub owner_word: bool,
}

impl Attribution {
    /// Whether this retirement names anything at all.
    #[must_use]
    pub fn names_something(&self) -> bool {
        self.round.is_some() || self.commit.is_some() || self.date.is_some() || self.owner_word
    }

    /// Whether what it names is something the world could not confirm.
    ///
    /// A FALSE NAME IS WORSE THAN NO NAME, so it refuses rather than falling
    /// back on the other names beside it: the claim "this was retired by that
    /// commit" is checkable and it is false.
    ///
    /// AND THE ROUND IS ASKED THE SAME WAY THE COMMIT IS (Round 1313). It was
    /// not, for the whole life of this rule — `round` was parsed, carried into
    /// every report and resolved against nothing, so a closure citing a round
    /// far past anything that exists — described rather than spelled, since
    /// `tools/*/src/` is scanned and spelling it would BE the citation — retired
    /// its row on sight. That is the identical defect `--repo` was made required
    /// for, on the identical argument: the arc's termination is a count, and a
    /// count that believes a name nobody checked can be reached by writing a
    /// sentence. Two names, one invariant — a field with two write paths and
    /// only one of them enforced is the shape this project's `CLAUDE.md`
    /// forbids, and here the two paths were two halves of one attribution.
    #[must_use]
    pub fn dangles(&self, unresolved: &Unresolved) -> bool {
        self.commit
            .as_ref()
            .is_some_and(|sha| unresolved.commits.contains(sha))
            || self
                .round
                .as_ref()
                .is_some_and(|round| unresolved.rounds.contains(round))
    }
}

/// The names this ledger's retirements gave that the world could not confirm.
///
/// ONE TYPE BECAUSE IT IS ONE QUESTION. The commit axis passed a bare set of
/// shas through five signatures, and adding the round axis beside it as a second
/// bare set would have made every one of them take two anonymous
/// `BTreeSet<String>` arguments in an order nothing but a comment defends. What
/// a caller owes this library is "here is what did not resolve", and the reason
/// each name failed belongs to the name, not to the argument position.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Unresolved {
    /// Shas a retirement named that the repository does not have.
    pub commits: BTreeSet<String>,
    /// Rounds a retirement named, spelled as the ledger spells them, that the
    /// atomic store does not have.
    pub rounds: BTreeSet<String>,
}

impl Unresolved {
    /// Whether every name this ledger gave resolved.
    ///
    /// NO `len` BESIDE IT, and that is the rule R1310 landed rather than an
    /// oversight: the only caller asks whether anything failed, the report
    /// prints the two axes apart because their repairs differ, and a total
    /// nobody names would be a method this crate carries for the shape of the
    /// pair rather than for a reader.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commits.is_empty() && self.rounds.is_empty()
    }
}

/// Where this line APPLIES the retirement word, and what it names — or `None`
/// when the word is only being spoken about.
///
/// TWO WAYS A LINE HOLDS THE WORD WITHOUT RETIRING ANYTHING, both measured:
///   - INSIDE A CODE SPAN, where the word is the subject. This crate's own rule
///     is prose about `CLOSED`, and writing that prose in the ledger deleted the
///     row it was written in.
///   - WITH NOTHING ATTRIBUTING IT, which is prose that happens to use the word.
fn retirement_on(line: &str) -> Option<(usize, Attribution)> {
    let at = retirement_at(line)?;
    Some((at, attribution_on(line)))
}

/// The first use of the retirement word that is not inside a quotation.
///
/// TWO NOTATIONS BECAUSE THIS LEDGER QUOTES IN TWO WAYS, and both were learned
/// by being bitten. A `code span` was the first, and it was not enough: the row
/// registering this very defect quoted a whole retirement — `「(CLOSED
/// 2026-08-14)」` — in the corner brackets this ledger uses for quoting prose,
/// and retired itself for the SECOND time. A rule that makes an author remember
/// which of two quotation marks is safe is a rule that will be forgotten.
///
/// AN UNCLOSED DELIMITER MAKES THE REST OF THE LINE A QUOTATION, which can only
/// make this MISS a retirement — the row then stays open and loud, never
/// quietly retired. That is the direction this is willing to be wrong in.
fn retirement_at(line: &str) -> Option<usize> {
    let mut in_code = false;
    let mut quoted = 0usize;
    for (offset, c) in line.char_indices() {
        match c {
            '`' => {
                in_code = !in_code;
                continue;
            }
            '\u{300C}' => {
                quoted += 1;
                continue;
            }
            '\u{300D}' => {
                quoted = quoted.saturating_sub(1);
                continue;
            }
            _ => {}
        }
        if !in_code && quoted == 0 && line[offset..].starts_with(RETIREMENT) {
            return Some(offset);
        }
    }
    None
}

/// What a line names beside its retirement word.
///
/// THE WHOLE LINE IS THE WINDOW because this ledger writes the name on both
/// sides: `CLOSED (R1297, 커밋 …)` after, and `| ~~N123~~ | R1244 CLOSED |`
/// before. It is also the granularity `retired` already reads at.
fn attribution_on(line: &str) -> Attribution {
    Attribution {
        round: round_in(line),
        // THE WORD IS THE DISCRIMINATOR, NOT THE BACKTICKS. `31387185994` is a
        // run id and reads as hexadecimal; a rule that took any backticked hex
        // would call GitHub run ids dangling commits, which is a gate that
        // reddens on things that are not its subject — the kind people disable.
        commit: ["커밋", "commit"]
            .iter()
            .find_map(|word| sha_after(line, word)),
        date: date_in(line),
        owner_word: line.contains("사장님 워드") || line.contains("owner's word"),
    }
}

/// The `YYYY-MM-DD` on this line, if it carries one.
fn date_in(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    let shaped = |at: usize| {
        let digits = [0, 1, 2, 3, 5, 6, 8, 9];
        let dashes = [4, 7];
        digits
            .iter()
            .all(|o| bytes.get(at + o).is_some_and(u8::is_ascii_digit))
            && dashes.iter().all(|o| bytes.get(at + o) == Some(&b'-'))
    };
    (0..bytes.len())
        .find(|at| shaped(*at))
        .map(|at| line[at..at + 10].to_string())
}

/// The backticked sha spelled immediately after `word`, if it is one.
fn sha_after(line: &str, word: &str) -> Option<String> {
    let at = line.find(word)? + word.len();
    let rest = line[at..].trim_start().strip_prefix('`')?;
    let end = rest.find('`')?;
    let sha = &rest[..end];
    let spelled_like_one =
        (7..=40).contains(&sha.len()) && sha.chars().all(|c| c.is_ascii_hexdigit());
    spelled_like_one.then(|| sha.to_string())
}

/// The round citation on this line, if any.
fn round_in(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    for at in 0..bytes.len() {
        if bytes[at] != b'R' {
            continue;
        }
        // NOT PART OF A LONGER WORD, so `RED` inside `REDACTED` and the `R` of a
        // capitalised sentence do not become round numbers.
        if at > 0 && bytes[at - 1].is_ascii_alphanumeric() {
            continue;
        }
        let mut end = at + 1;
        if line[at..].starts_with("Round ") {
            end = at + "Round ".len();
        }
        let digits_from = end;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        if end > digits_from {
            return Some(line[at..end].to_string());
        }
    }
    None
}

/// Every commit this ledger's retirements name, and the line each was named on.
///
/// SEPARATE FROM RESOLVING THEM, because resolving needs a repository and this
/// library reads text. The caller asks git and hands back what did not resolve.
#[must_use]
pub fn commits_named_by_retirements(ledger: &str) -> BTreeMap<String, usize> {
    named_by_retirements(ledger, |attribution| attribution.commit)
}

/// Every round this ledger's retirements name, and the line each was named on.
///
/// THE OTHER HALF OF THE SAME CHECK (Round 1313), and it is a separate function
/// for the reason its neighbour is: resolving a round means asking the atomic
/// store, which is a program, and this library reads text.
///
/// SPELLED AS THE LEDGER SPELLS IT — `R1299` and `Round 1299` both occur, and
/// the key here is what `Attribution.round` holds so that a caller's answer can
/// be matched against it without a second normalisation rule free to drift from
/// the first. Turning the spelling into the store's key is the caller's job,
/// because the store's key shape is the store's business and not this reader's.
#[must_use]
pub fn rounds_named_by_retirements(ledger: &str) -> BTreeMap<String, usize> {
    named_by_retirements(ledger, |attribution| attribution.round)
}

/// The one walk both name-collectors are.
///
/// LIFTED OUT SO THE TWO AXES CANNOT DISAGREE ABOUT WHAT A RETIREMENT IS. This
/// crate has already paid for the same six letters meaning two things one
/// function apart; two copies of "walk the lines, take the attributions" would
/// be the same bill arriving again.
fn named_by_retirements(
    ledger: &str,
    name: impl Fn(Attribution) -> Option<String>,
) -> BTreeMap<String, usize> {
    let mut named = BTreeMap::new();
    for (number, line) in ledger.lines().enumerate() {
        if let Some((_, attribution)) = retirement_on(line) {
            if let Some(given) = name(attribution) {
                named.entry(given).or_insert(number + 1);
            }
        }
    }
    named
}

/// Every id this ledger says is retired, by either notation.
///
/// TWO NOTATIONS BECAUSE THE LEDGER HAS TWO. Prose retires a row with the word
/// `CLOSED` beside the id — sometimes a run of ids joined by `·`, all retired by
/// one round — and the tables retire it by striking the id through. A reader of
/// only the first calls thirty struck rows open; a reader of only the second
/// calls every prose retirement open.
///
/// `unresolved` holds the names a retirement gave that the world could not
/// confirm — shas this repository does not have (R1298) and rounds the atomic
/// store does not have (Round 1313). A retirement naming one of them does not
/// retire anything.
#[must_use]
pub fn retired(ledger: &str, unresolved: &Unresolved) -> BTreeSet<String> {
    let mut closed = BTreeSet::new();
    // `~~N123~~` — the tables' notation, and the one the prose reader missed.
    let mut rest = ledger;
    while let Some(open) = rest.find("~~") {
        let after = &rest[open + 2..];
        if let Some(shut) = after.find("~~") {
            let inner = &after[..shut];
            if id_at(inner.as_bytes(), 0) == Some(inner.len()) {
                closed.insert(inner.to_string());
            }
            rest = &after[shut + 2..];
        } else {
            break;
        }
    }
    // `N220·N221·N222 CLOSED` — a run of ids retired by one round, where a
    // reader that took only the nearest id retires the last of three.
    for line in ledger.lines() {
        // THE SAME RESOLVER THE ROW PATH USES (R1298). This one was already the
        // careful reader; what it lacked was the two refusals below, and having
        // them in one place is what stops the pair drifting again.
        let Some((word, attribution)) = retirement_on(line) else {
            continue;
        };
        if !attribution.names_something() || attribution.dangles(unresolved) {
            continue;
        }
        closed.extend(run_reaching(line, word));
    }
    closed
}

/// The ids a retirement at `word` applies to.
///
/// ONLY THE RUN THAT REACHES THE WORD, so a sentence naming five debts and then
/// closing a sixth does not retire all six. The run ends where anything but a
/// separator sits between an id and the next.
///
/// LIFTED OUT SO THE REFUSAL READER CAN ASK THE SAME QUESTION (R1298): "which
/// ids did this line mean" must not have two answers depending on whether the
/// line was accepted.
fn run_reaching(line: &str, word: usize) -> Vec<String> {
    let before = &line[..word];
    let mut ids = Vec::new();
    let bytes = before.as_bytes();
    let mut at = 0usize;
    while at < bytes.len() {
        if let Some(width) = id_at(bytes, at) {
            ids.push((at, before[at..at + width].to_string()));
            at += width;
        } else {
            at += 1;
        }
    }
    let mut run: Vec<String> = Vec::new();
    let mut previous_end: Option<usize> = None;
    for (at, id) in ids.into_iter().rev() {
        let end = at + id.len();
        if let Some(previous) = previous_end {
            let between = &before[end..previous];
            if between.chars().any(|c| c.is_ascii_alphanumeric()) {
                break;
            }
        } else if before[end..].chars().any(|c| c.is_ascii_alphanumeric()) {
            // Between the last id and the word there is a word, so this line
            // closes something other than these ids.
            break;
        }
        run.push(id);
        previous_end = Some(at);
    }
    run
}

/// Why a retirement this ledger wrote was not counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// It named no round, commit or owner's word.
    NamesNothing,
    /// It named a commit this repository does not have.
    NamesAMissingCommit,
    /// It named a round the atomic store does not have (Round 1313).
    ///
    /// WHICH INCLUDES A ROUND BELOW 252 and that is not an oversight: rounds
    /// 1-251 are the off-main legacy-migration closure, they are not in the
    /// store, and `CLAUDE.md` says a citation to one cannot be verified here and
    /// must not be written as though it were. So a retirement resting on one is
    /// refused with the rest, and the repair is the same — name the round that
    /// actually closed the row, or the commit.
    NamesAMissingRound,
}

/// Every id whose retirement this reader refused, and is retired nowhere else.
///
/// A COUNT THAT CHANGES WITHOUT SAYING SO IS THE DEFECT THIS CRATE EXISTS FOR.
/// Tightening the reader moved two ids out of the retired set on the real
/// ledger, and neither sits in the autonomous branch — so the walk below would
/// never have shown them, and the census would have reported a different number
/// with nothing to explain it. A refusal that only shows up as arithmetic is a
/// refusal nobody reads.
#[must_use]
pub fn refused_retirements(
    ledger: &str,
    unresolved: &Unresolved,
) -> BTreeMap<String, (usize, Refusal)> {
    let accepted = retired(ledger, unresolved);
    let mut refused: BTreeMap<String, (usize, Refusal)> = BTreeMap::new();
    for (number, line) in ledger.lines().enumerate() {
        let Some((word, attribution)) = retirement_on(line) else {
            continue;
        };
        // WHICH NAME FAILED IS PART OF THE FINDING (Round 1313). A row refused
        // for a sha and a row refused for a round need different repairs, and a
        // reader told only "refused" has to go and work out which — the shape of
        // report this crate exists to stop being the answer.
        let dangling_commit = attribution
            .commit
            .as_ref()
            .is_some_and(|sha| unresolved.commits.contains(sha));
        let why = if dangling_commit {
            Refusal::NamesAMissingCommit
        } else if attribution.dangles(unresolved) {
            Refusal::NamesAMissingRound
        } else if !attribution.names_something() {
            Refusal::NamesNothing
        } else {
            continue;
        };
        for id in run_reaching(line, word) {
            if !accepted.contains(&id) {
                refused.entry(id).or_insert((number + 1, why));
            }
        }
    }
    refused
}

/// Whether this ledger says the debt arc is finished.
///
/// THE PREDICATE IS THE LIBRARY'S BECAUSE IT IS THE ANSWER (R1299). It was three
/// conditions written inline in `main.rs`, where nothing but a person could ask
/// it — and this is the one question the whole arc terminates on.
///
/// AND A REFUSAL BLOCKS IT NOW, which reverses the line R1298 drew one round
/// earlier. That round let an unattributed retirement print without blocking,
/// on the argument that a gate taking the arc hostage over notation would be an
/// exemption-shaped mistake pointing the other way. The argument was made
/// WITHOUT KNOWING THE POPULATION. Measured, it is ONE row — `N147`, whose
/// closing round is written six lines above it in the ledger's own history —
/// and one row that a single line repairs is not hostage-taking, it is a
/// gate whose zero is reachable. An advisory line in a program is prose, and
/// prose is what this crate exists to stop being the answer.
#[must_use]
pub fn finished(ledger: &str, unresolved: &Unresolved) -> bool {
    open_autonomous(ledger, unresolved).is_empty()
        && unresolved.is_empty()
        && refused_retirements(ledger, unresolved).is_empty()
}

/// What the ledger holds open under the autonomous branch.
///
/// THE ANSWER IS ROWS AND NOT A NUMBER, because a count nobody can open is a
/// count nobody checks. Each row comes back with the line it was registered on.
#[must_use]
pub fn open_autonomous(ledger: &str, unresolved: &Unresolved) -> Vec<Registration> {
    let closed = retired(ledger, unresolved);
    let all = registrations(ledger);
    // A ROW IS CLOSED IF ANY OF ITS REGISTRATIONS SAYS SO, including a bullet
    // whose own body carries the word — one place saying a debt is retired is
    // enough, and the ledger writes the retirement wherever the round was.
    //
    // THROUGH THE SAME RESOLVER AS THE LINE PATH ABOVE (R1298). This read
    // `body.contains("CLOSED")`, which retires a row for SAYING the word: the
    // row registering that very defect named it in its own headline and
    // vanished from the census, and the census reported a smaller number with
    // nothing to show that it had.
    let mut says_closed: BTreeSet<String> = closed;
    for row in &all {
        if row.shape != Shape::Bullet {
            continue;
        }
        let retires = row.body.lines().any(|line| {
            retirement_on(line).is_some_and(|(_, attribution)| {
                attribution.names_something() && !attribution.dangles(unresolved)
            })
        });
        if retires {
            says_closed.insert(row.id.clone());
        }
    }
    let mut first: BTreeMap<String, Registration> = BTreeMap::new();
    for row in all {
        if !row.is_autonomous() || row.is_a_mention() || says_closed.contains(&row.id) {
            continue;
        }
        first.entry(row.id.clone()).or_insert(row);
    }
    let mut rows: Vec<Registration> = first.into_values().collect();
    rows.sort_by_key(|row| {
        (
            row.id
                .chars()
                .filter(char::is_ascii_digit)
                .collect::<String>()
                .parse::<u32>()
                .unwrap_or(0),
            row.id.clone(),
        )
    });
    rows
}
