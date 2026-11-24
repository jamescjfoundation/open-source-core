// SPDX-License-Identifier: MIT OR Apache-2.0
//! Card text helpers: short previews, and simple lists in a card's body.

/// `text` cut to about `max` characters at a word, with an ellipsis when cut.
pub fn preview(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max).collect();
    let at = cut.rfind(' ').unwrap_or(cut.len());
    format!("{}…", cut[..at].trim_end())
}

/// A card's heading: two lines' worth at most.
pub fn heading_preview(text: &str) -> String {
    if text.chars().count() <= 52 {
        return text.to_string();
    }
    format!("{}…", text.chars().take(50).collect::<String>().trim_end())
}

/// `body` with a new list line: a bullet, or number `n`.
pub fn with_list_line(body: &str, n: Option<usize>) -> String {
    let mut out = body.trim_end_matches('\n').to_string();
    if !out.is_empty() {
        out.push('\n');
    }
    match n {
        Some(n) => out.push_str(&format!("{n}. ")),
        None => out.push_str("• "),
    }
    out
}

/// The number after the last numbered line of `body` (1 when there is none).
pub fn next_number(body: &str) -> usize {
    body.lines().rev().find_map(|l| l.split_once(". ").and_then(|(n, _)| n.trim().parse::<usize>().ok())).map_or(1, |n| n + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_cut_at_a_word() {
        assert_eq!(preview("short", 10), "short");
        assert_eq!(preview("one two three four", 10), "one two…");
        assert_eq!(preview("abcdefghijkl", 5), "abcde…");
        let long = "x".repeat(60);
        assert_eq!(heading_preview(&long).chars().count(), 51);
    }

    #[test]
    fn list_lines_number_on() {
        assert_eq!(with_list_line("", None), "• ");
        assert_eq!(with_list_line("1. First\n", Some(next_number("1. First\n"))), "1. First\n2. ");
        assert_eq!(next_number("Notes"), 1);
    }
}
