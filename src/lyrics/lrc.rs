//! LRC parser: `[mm:ss.xx] text`, multiple timestamps per line, `hh:mm:ss.xx`
//! timestamps, `[offset:±ms]` tags, and other metadata.

#[derive(Debug, Clone, PartialEq)]
pub struct LyricLine {
    pub time_ms: i64,
    pub text: String,
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
        let text = strip_word_tags(&text);
        for time_ms in stamps {
            timed.push(LyricLine {
                time_ms,
                text: text.clone(),
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

fn strip_word_tags(text: &str) -> String {
    // Some LRCLIB records use enhanced LRC with per-word tags:
    // `<00:18.89>We're <00:19.25>no ...`. For a per-line display the word
    // tags are stripped and the spacing is tidied up.
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut removed = false;

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
                removed = true;
                continue;
            }
            out.push('<');
            out.push_str(&inner);
            if closed {
                out.push('>');
            }
        } else {
            out.push(ch);
        }
    }

    if removed {
        collapse_spaces(&out)
    } else {
        out
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
    fn finds_active_line() {
        let lines = vec![
            LyricLine {
                time_ms: 1_000,
                text: "a".into(),
            },
            LyricLine {
                time_ms: 2_000,
                text: "b".into(),
            },
            LyricLine {
                time_ms: 3_000,
                text: "c".into(),
            },
        ];
        assert_eq!(active_index(&lines, 0), None);
        assert_eq!(active_index(&lines, 1_500), Some(0));
        assert_eq!(active_index(&lines, 3_000), Some(2));
        assert_eq!(active_index(&lines, 99_000), Some(2));
    }
}
