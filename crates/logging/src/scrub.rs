//! What a log line may not carry: the owner's home folder, which on Windows
//! is named after them, and a phone number.
//!
//! Most of the tree already logs addresses through `Jid::observe`, which turns
//! a number into a keyed token. This is the net under the lines that do not:
//! a path printed with `display()`, an error string from a library that quotes
//! its input, a number typed by hand. Applied once, where every line is
//! written, so a new log call cannot forget it.

/// The line with the home folder replaced by `~` and phone numbers masked.
#[must_use]
pub fn scrub(line: &str, home: Option<&str>) -> String {
    let mut out = match home.filter(|home| home.len() > 3) {
        Some(home) => replace_home(line, home),
        None => line.to_string(),
    };
    out = mask_numbers(&out);
    out
}

/// Every spelling of the home folder a log is likely to hold: as given, with
/// the other slash, and in either case for the drive letter's sake.
fn replace_home(line: &str, home: &str) -> String {
    let forward = home.replace('\\', "/");
    let mut out = line.to_string();
    for spelling in [home.to_string(), forward] {
        out = replace_ignore_ascii_case(&out, &spelling, "~");
    }
    out
}

fn replace_ignore_ascii_case(haystack: &str, needle: &str, with: &str) -> String {
    if needle.is_empty() {
        return haystack.to_string();
    }
    let lower = haystack.to_ascii_lowercase();
    let needle_lower = needle.to_ascii_lowercase();
    let mut out = String::with_capacity(haystack.len());
    let mut from = 0;
    while let Some(found) = lower[from..].find(&needle_lower) {
        let at = from + found;
        out.push_str(&haystack[from..at]);
        out.push_str(with);
        from = at + needle.len();
    }
    out.push_str(&haystack[from..]);
    out
}

/// A run of 7 to 15 digits that is written as a phone number: after a `+`,
/// or before `@s.whatsapp.net` / `@c.us`. All but the last two digits go.
/// A bare run of digits is left alone: timestamps, sizes and message ids are
/// what most of those are, and a log without them cannot be read.
fn mask_numbers(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() && (i == 0 || !bytes[i - 1].is_ascii_digit()) {
            let start = i;
            let mut end = i;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            let digits = end - start;
            let rest = &line[end..];
            let device_suffix = rest.strip_prefix(':').map_or(rest, |after| {
                after.trim_start_matches(|c: char| c.is_ascii_digit())
            });
            let is_phone = (7..=15).contains(&digits)
                && ((start > 0 && bytes[start - 1] == b'+')
                    || device_suffix.starts_with("@s.whatsapp.net")
                    || device_suffix.starts_with("@c.us"));
            if is_phone {
                out.push_str(&"•".repeat(digits - 2));
                out.push_str(&line[end - 2..end]);
            } else {
                out.push_str(&line[start..end]);
            }
            i = end;
            continue;
        }
        let ch = line[i..].chars().next().unwrap_or_default();
        out.push(ch);
        i += ch.len_utf8().max(1);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::scrub;

    #[test]
    fn the_home_folder_is_never_written() {
        let home = r"C:\Users\ana";
        assert_eq!(
            scrub(r"saved to C:\Users\ana\Downloads\a.pdf", Some(home)),
            r"saved to ~\Downloads\a.pdf"
        );
        assert_eq!(
            scrub("opened c:/users/ANA/AppData/Local/velo", Some(home)),
            "opened ~/AppData/Local/velo"
        );
    }

    #[test]
    fn phone_numbers_are_masked_and_other_numbers_kept() {
        assert_eq!(
            scrub("from 5511999990001@s.whatsapp.net", None),
            "from •••••••••••01@s.whatsapp.net"
        );
        assert_eq!(
            scrub("dial +5511999990001 now", None),
            "dial +•••••••••••01 now"
        );
        assert_eq!(
            scrub("device 5511999990001:12@s.whatsapp.net", None),
            "device •••••••••••01:12@s.whatsapp.net"
        );
        // A timestamp, a size, a LID and a message id are not phone numbers.
        assert_eq!(
            scrub(
                "at 1764000000000 took 2048 bytes 123456789@lid 3EB0ABC",
                None
            ),
            "at 1764000000000 took 2048 bytes 123456789@lid 3EB0ABC"
        );
    }

    #[test]
    fn text_outside_ascii_survives() {
        assert_eq!(
            scrub("café ✓ +5511999990001", None),
            "café ✓ +•••••••••••01"
        );
    }
}
