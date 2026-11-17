//! LRC parser: `[mm:ss.xx] text`, multiple timestamps per line, `hh:mm:ss.xx`
//! timestamps, `[offset:±ms]` tags, and other metadata.

#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub time_ms: i64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LyricLine {
    pub time_ms: i64,
    pub text: String,
    /// Per-word timestamps from enhanced LRC (`<mm:ss.xx>word`), empty when
    /// the source only has line-level timing.
    pub words: Vec<Word>,
}

#[derive(Debug, Clone, Default)]
pub struct Lrc {
    pub lines: Vec<LyricLine>,
    pub offset_ms: i64,
}

pub fn parse(raw: &str) -> Lrc {
    let mut lrc = Lrc::default();
    let mut timed: Vec<LyricLine> = Vec::new();

    for line in raw.lines() {
        let line = line.trim_end_matches('\r');
        let (stamps, text, offset) = split_tags(line);
        if let Some(offset) = offset {
            lrc.offset_ms = offset;
        }
        let text = text.trim().to_string();
        let (text, words) = parse_word_tags(&text);
        for time_ms in stamps {
            timed.push(LyricLine {
                time_ms,
                text: text.clone(),
                words: words.clone(),
            });
        }
    }

    timed.sort_by_key(|line| line.time_ms);
    timed.dedup_by(|a, b| a.time_ms == b.time_ms && a.text == b.text);
    lrc.lines = timed;
    lrc
}

fn split_tags(line: &str) -> (Vec<i64>, String, Option<i64>) {
    let mut stamps = Vec::new();
    let mut offset = None;
    let mut rest = line.trim_start();

    while let Some(after_open) = rest.strip_prefix('[') {
        let Some(close) = after_open.find(']') else {
            break;
        };
        let tag = &after_open[..close];

        if let Some(ms) = parse_stamp(tag) {
            stamps.push(ms);
            rest = &after_open[close + 1..];
            continue;
        }

        if let Some((key, value)) = tag.split_once(':') {
            let key = key.trim().to_ascii_lowercase();
            if matches!(
                key.as_str(),
                "offset" | "ar" | "ti" | "al" | "by" | "re" | "ve" | "length"
            ) {
                if key == "offset" {
                    offset = value.trim().parse::<i64>().ok();
                }
                rest = &after_open[close + 1..];
                continue;
            }
        }

        // Not a known tag (for example `[Chorus]`): keep it as lyric text.
        break;
    }

    (stamps, rest.trim_start().to_string(), offset)
}

/// Parses enhanced-LRC per-word tags (`<00:18.89>We're <00:19.25>no ...`)
/// into a clean display string plus the list of `(word, timestamp)` pairs.
/// Lines without word tags return an empty `words` vec — the caller falls
/// back to line-level timing in that case.
fn parse_word_tags(text: &str) -> (String, Vec<Word>) {
    let mut out = String::with_capacity(text.len());
    let mut words = Vec::new();
    let mut chars = text.chars().peekable();
    let mut pending_time: Option<i64> = None;
    let mut pending_word = String::new();
    let mut found_any = false;

    let flush_word =
        |out: &str, pending_time: Option<i64>, pending_word: &str, words: &mut Vec<Word>| {
            if let Some(time_ms) = pending_time {
                let word = pending_word.trim();
                if !word.is_empty() {
                    words.push(Word {
                        time_ms,
                        text: word.to_string(),
                    });
                }
            }
            let _ = out;
        };

    while let Some(ch) = chars.next() {
        if ch == '<' {
            let mut inner = String::new();
            let mut closed = false;
            for next in chars.by_ref() {
                if next == '>' {
                    closed = true;
                    break;
                }
                inner.push(next);
            }
            if closed && parse_stamp(inner.trim()).is_some() {
                // A new word tag starts: close out whatever word was pending
                // under the previous timestamp.
                flush_word(&out, pending_time, &pending_word, &mut words);
                pending_word.clear();
                pending_time = parse_stamp(inner.trim());
                found_any = true;
                continue;
            }
            out.push('<');
            out.push_str(&inner);
            if closed {
                out.push('>');
            }
            pending_word.push('<');
            pending_word.push_str(&inner);
            if closed {
                pending_word.push('>');
            }
        } else {
            out.push(ch);
            pending_word.push(ch);
        }
    }
    flush_word(&out, pending_time, &pending_word, &mut words);

    if found_any {
        (collapse_spaces(&out), words)
    } else {
        (out, Vec::new())
    }
}

fn collapse_spaces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut previous_space = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !previous_space {
                out.push(' ');
                previous_space = true;
            }
        } else {
            out.push(ch);
            previous_space = false;
        }
    }
    out.trim().to_string()
}

fn parse_stamp(tag: &str) -> Option<i64> {
    let parts: Vec<&str> = tag.split(':').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return None;
    }

    let parse_u = |value: &str| -> Option<i64> {
        if value.is_empty() || !value.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        value.parse().ok()
    };

    let (hours, minutes, seconds) = match parts.as_slice() {
        [m, rest] => (0, parse_u(m)?, *rest),
        [h, m, rest] => (parse_u(h)?, parse_u(m)?, *rest),
        _ => return None,
    };

    let (whole, fraction) = match seconds.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (seconds, None),
    };
    let seconds = parse_u(whole)?;

    let mut millis = 0i64;
    if let Some(fraction) = fraction {
        let mut scale = 100i64;
        for ch in fraction.chars().take(3) {
            let digit = ch.to_digit(10)? as i64;
            millis += digit * scale;
            scale /= 10;
        }
    }

    Some(hours * 3_600_000 + minutes * 60_000 + seconds * 1000 + millis)
}

/// Index of the last line whose timestamp is <= the given position.
pub fn active_index(lines: &[LyricLine], pos_ms: i64) -> Option<usize> {
    if lines.is_empty() {
        return None;
    }
    let index = match lines.binary_search_by(|line| line.time_ms.cmp(&pos_ms)) {
        Ok(index) => index,
        Err(0) => return None,
        Err(index) => index - 1,
    };
    Some(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_tags() {
        let lrc = parse("[00:12.34] Hello\n[00:15.00][00:20.00] Repeat\n[ar:Someone]");
        assert_eq!(lrc.lines.len(), 3);
        assert_eq!(lrc.lines[0].time_ms, 12_340);
        assert_eq!(lrc.lines[0].text, "Hello");
        assert_eq!(lrc.lines[1].time_ms, 15_000);
        assert_eq!(lrc.lines[2].time_ms, 20_000);
        assert_eq!(lrc.lines[1].text, "Repeat");
    }

    #[test]
    fn parses_hours_offset_and_fraction_variants() {
        let lrc = parse("[offset:+250]\n[01:02:03.5] Long\n[00:01] Short");
        assert_eq!(lrc.offset_ms, 250);
        assert_eq!(lrc.lines[0].time_ms, 1_000);
        assert_eq!(lrc.lines[1].time_ms, 3_723_500);
    }

    #[test]
    fn keeps_bracket_lyrics() {
        let lrc = parse("[00:01.00] [Chorus]\n[00:02.00] Normal");
        assert_eq!(lrc.lines.len(), 2);
        assert_eq!(lrc.lines[0].text, "[Chorus]");
    }

    #[test]
    fn strips_word_level_tags() {
        let lrc = parse("[00:18.89] <00:18.89>We're <00:19.25>no <00:19.53>strangers");
        assert_eq!(lrc.lines.len(), 1);
        assert_eq!(lrc.lines[0].text, "We're no strangers");
    }

    #[test]
    fn word_level_tags_are_kept_as_per_word_timestamps() {
        let lrc = parse("[00:18.89] <00:18.89>We're <00:19.25>no <00:19.53>strangers");
        let words = &lrc.lines[0].words;
        assert_eq!(words.len(), 3);
        assert_eq!(
            words[0],
            Word {
                time_ms: 18_890,
                text: "We're".to_string()
            }
        );
        assert_eq!(
            words[1],
            Word {
                time_ms: 19_250,
                text: "no".to_string()
            }
        );
        assert_eq!(
            words[2],
            Word {
                time_ms: 19_530,
                text: "strangers".to_string()
            }
        );
    }

    #[test]
    fn lines_without_word_tags_have_an_empty_words_list() {
        let lrc = parse("[00:01.00] Plain line, no word timing");
        assert!(lrc.lines[0].words.is_empty());
    }

    #[test]
    fn finds_active_line() {
        let lines = vec![
            LyricLine {
                time_ms: 1_000,
                text: "a".into(),
                words: Vec::new(),
            },
            LyricLine {
                time_ms: 2_000,
                text: "b".into(),
                words: Vec::new(),
            },
            LyricLine {
                time_ms: 3_000,
                text: "c".into(),
                words: Vec::new(),
            },
        ];
        assert_eq!(active_index(&lines, 0), None);
        assert_eq!(active_index(&lines, 1_500), Some(0));
        assert_eq!(active_index(&lines, 3_000), Some(2));
        assert_eq!(active_index(&lines, 99_000), Some(2));
    }

    #[test]
    fn negative_offset_is_parsed() {
        let lrc = parse("[offset:-150]\n[00:01.00] Line");
        assert_eq!(lrc.offset_ms, -150);
    }

    #[test]
    fn blank_and_whitespace_only_lines_are_ignored() {
        let lrc = parse("[00:01.00] First\n\n   \n[00:02.00] Second");
        assert_eq!(lrc.lines.len(), 2);
        assert_eq!(lrc.lines[0].text, "First");
        assert_eq!(lrc.lines[1].text, "Second");
    }

    #[test]
    fn metadata_tags_are_stripped_and_do_not_produce_lines() {
        let lrc = parse("[ar:Someone][ti:Title][al:Album][by:Author][re:App][ve:1.0][length:03:30]\n[00:01.00] Lyric");
        assert_eq!(lrc.lines.len(), 1);
        assert_eq!(lrc.lines[0].text, "Lyric");
    }

    #[test]
    fn duplicate_timestamp_and_text_pairs_are_deduplicated() {
        let lrc = parse("[00:01.00] Same\n[00:01.00] Same");
        assert_eq!(lrc.lines.len(), 1);
    }

    #[test]
    fn lines_are_sorted_by_timestamp_regardless_of_file_order() {
        let lrc = parse("[00:05.00] Later\n[00:01.00] Earlier");
        assert_eq!(lrc.lines[0].text, "Earlier");
        assert_eq!(lrc.lines[1].text, "Later");
    }

    #[test]
    fn word_tags_with_trailing_unclosed_tag_are_left_intact() {
        // A malformed enhanced-LRC line (missing closing `>`) must not corrupt
        // or silently swallow the rest of the text.
        let lrc = parse("[00:01.00] <00:01.00>Hello <unterminated");
        assert_eq!(lrc.lines[0].text, "Hello <unterminated");
    }
}
