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

/// Every id this ledger says is retired, by either notation.
///
/// TWO NOTATIONS BECAUSE THE LEDGER HAS TWO. Prose retires a row with the word
/// `CLOSED` beside the id — sometimes a run of ids joined by `·`, all retired by
/// one round — and the tables retire it by striking the id through. A reader of
/// only the first calls thirty struck rows open; a reader of only the second
/// calls every prose retirement open.
#[must_use]
pub fn retired(ledger: &str) -> BTreeSet<String> {
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
        let Some(word) = line.find("CLOSED") else {
            continue;
        };
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
        // ONLY THE RUN THAT REACHES THE WORD, so a sentence naming five debts
        // and then closing a sixth does not retire all six. The run ends where
        // anything but a separator sits between an id and the next.
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
                // Between the last id and `CLOSED` there is a word, so this line
                // closes something other than these ids.
                break;
            }
            run.push(id);
            previous_end = Some(at);
        }
        closed.extend(run);
    }
    closed
}

/// What the ledger holds open under the autonomous branch.
///
/// THE ANSWER IS ROWS AND NOT A NUMBER, because a count nobody can open is a
/// count nobody checks. Each row comes back with the line it was registered on.
#[must_use]
pub fn open_autonomous(ledger: &str) -> Vec<Registration> {
    let closed = retired(ledger);
    let all = registrations(ledger);
    // A ROW IS CLOSED IF ANY OF ITS REGISTRATIONS SAYS SO, including a bullet
    // whose own body carries the word — one place saying a debt is retired is
    // enough, and the ledger writes the retirement wherever the round was.
    let mut says_closed: BTreeSet<String> = closed;
    for row in &all {
        if row.shape == Shape::Bullet && row.body.contains("CLOSED") {
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
