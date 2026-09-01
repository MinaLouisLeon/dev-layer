//! Getting a path out of a diff header line.
//!
//! Harder than it looks, and worth its own file because every case here was
//! found by running real git rather than reading the manual.
//!
//! ```text
//! --- a/src/main.rs           an ordinary path
//! +++ b/release notes.md<TAB> a path containing a space: git appends a TAB
//! --- /dev/null               the file did not exist on this side
//! +++ "b/od\303\251.md"       C-quoted, when quotepath is on or the name is odd
//! ```
//!
//! The trailing tab is the important one. `diff --git a/release notes.md b/release
//! notes.md` is genuinely ambiguous - there is no way to tell where the first
//! path ends - which is why this reads the `---`/`+++` lines instead, where the
//! path runs to the end of the line and git marks it with a tab when it had to.

/// Git's stand-in for "there is no file on this side".
pub const DEV_NULL: &str = "/dev/null";

/// Takes the path out of a `--- ` or `+++ ` line, already stripped of its
/// marker. `None` for `/dev/null`, which is an absence rather than a name.
pub fn from_header(rest: &str) -> Option<String> {
    // The tab git appends when the path contains a space. Only ever trailing,
    // so trimming the end is enough and a tab inside a name survives.
    let value = rest.trim_end_matches('\t');
    if value == DEV_NULL {
        return None;
    }
    let unquoted = unquote(value);
    // `a/` and `b/` are git's prefixes, not part of the name. A file genuinely
    // called `a/x` still arrives as `a/a/x`, so stripping exactly one is right.
    let stripped = unquoted
        .strip_prefix("a/")
        .or_else(|| unquoted.strip_prefix("b/"))
        .unwrap_or(&unquoted);
    (!stripped.is_empty()).then(|| stripped.to_string())
}

/// Takes a path out of the `a/old b/new` pair on a `diff --git` line.
///
/// The ambiguous one, and used only as a fallback: `a/release notes.md b/release
/// notes.md` cannot be split by looking for a space. It is still needed,
/// because a **binary file** and a **mode-only change** have no `---`/`+++`
/// lines at all and would otherwise arrive nameless and be dropped.
///
/// The way out is that the two halves are equal for everything except a
/// rename - so the split that makes them equal is the right one. A rename has
/// `rename to` to correct it afterwards.
pub fn from_pair(rest: &str) -> Option<String> {
    let stripped = rest.strip_prefix("a/")?;
    let symmetric = stripped
        .match_indices(" b/")
        .find(|(index, _)| stripped[..*index] == stripped[index + 3..]);
    let index = match symmetric {
        Some((index, _)) => index,
        // A rename: take the new side, which is the last ` b/`.
        None => stripped.rfind(" b/")?,
    };
    let value = unquote(&stripped[index + 3..]);
    (!value.is_empty()).then_some(value)
}

/// Takes the path out of a `rename from ` / `rename to ` line.
///
/// These carry no `a/` or `b/` prefix and no trailing tab, but they are quoted
/// under the same rules - and they are the *only* source of a path for a pure
/// rename, which git reports with no `---`/`+++` lines at all.
pub fn from_rename(rest: &str) -> Option<String> {
    let value = unquote(rest);
    (!value.is_empty()).then_some(value)
}

/// Undoes git's C-style quoting.
///
/// Git wraps a path in double quotes when it contains something it will not
/// print raw, and escapes the contents: `\t`, `\n`, `\"`, `\\`, and octal
/// `\303\251` for bytes. A value that is not quoted is returned untouched,
/// which is the common case now that every call passes `core.quotepath=false`.
pub fn unquote(value: &str) -> String {
    let Some(inner) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) else {
        return value.to_string();
    };

    let mut bytes: Vec<u8> = Vec::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            let mut buffer = [0u8; 4];
            bytes.extend_from_slice(ch.encode_utf8(&mut buffer).as_bytes());
            continue;
        }
        match chars.next() {
            Some('t') => bytes.push(b'\t'),
            Some('n') => bytes.push(b'\n'),
            Some('r') => bytes.push(b'\r'),
            Some('"') => bytes.push(b'"'),
            Some('\\') => bytes.push(b'\\'),
            // Octal: exactly three digits, and they are bytes rather than
            // characters - which is why this collects bytes and decodes once
            // at the end. A multi-byte character arrives as several escapes.
            Some(digit @ '0'..='7') => {
                let mut octal = String::from(digit);
                for _ in 0..2 {
                    match chars.clone().next() {
                        Some(next @ '0'..='7') => {
                            octal.push(next);
                            chars.next();
                        }
                        _ => break,
                    }
                }
                match u8::from_str_radix(&octal, 8) {
                    Ok(byte) => bytes.push(byte),
                    // Unreachable for real git output. A wrong path is a
                    // better outcome here than a panic.
                    Err(_) => bytes.extend_from_slice(octal.as_bytes()),
                }
            }
            Some(other) => {
                let mut buffer = [0u8; 4];
                bytes.extend_from_slice(other.encode_utf8(&mut buffer).as_bytes());
            }
            None => bytes.push(b'\\'),
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

#[cfg(test)]
mod shapes;
#[cfg(test)]
mod tests;
