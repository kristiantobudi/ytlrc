//! Membersihkan judul YouTube dan menebak pasangan artis/judul.

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedTrack {
    pub artist: String,
    pub track: String,
}

pub fn parse_title(raw: &str, channel: Option<&str>) -> ParsedTrack {
    let cleaned = clean_title(raw);
    let channel = channel.map(clean_channel).unwrap_or_default();

    let mut artist = String::new();
    let mut track = cleaned.clone();

    if let Some((left, right)) = split_pair(&cleaned) {
        let (artist_side, track_side) = choose_sides(left, right, &channel);
        artist = artist_side.to_string();
        track = track_side.to_string();
    }

    if artist.trim().is_empty() {
        artist = if channel.trim().is_empty() {
            "Unknown".to_string()
        } else {
            channel.clone()
        };
    }

    let track = tidy_piece(&track);
    ParsedTrack {
        artist: tidy_piece(&artist),
        track: if track.is_empty() { cleaned } else { track },
    }
}

pub fn clean_title(raw: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    let mut group = String::new();

    for ch in raw.chars() {
        match ch {
            '(' | '[' | '{' => {
                if depth == 0 {
                    group.clear();
                }
                depth += 1;
            }
            ')' | ']' | '}' => {
                if depth > 0 {
                    depth -= 1;
                    if depth == 0 {
                        if !is_noise_group(&group) {
                            out.push(' ');
                            out.push_str(&group);
                            out.push(' ');
                        }
                        group.clear();
                    }
                }
            }
            _ => {
                if depth > 0 {
                    group.push(ch);
                } else {
                    out.push(ch);
                }
            }
        }
    }

    if depth > 0 {
        out.push(' ');
        out.push_str(&group);
    }

    strip_trailing_noise(&tidy_piece(&out))
}

pub fn clean_channel(raw: &str) -> String {
    let mut value = raw.trim().to_string();
    for suffix in [" - topic", " – topic", " — topic", "topic"] {
        if value.to_lowercase().ends_with(suffix) {
            value.truncate(value.len() - suffix.len());
            break;
        }
    }
    if value.to_lowercase().ends_with("vevo") && value.len() > 4 {
        value.truncate(value.len() - 4);
    }
    tidy_piece(&value)
}

fn choose_sides<'a>(left: &'a str, right: &'a str, channel: &str) -> (&'a str, &'a str) {
    let channel_norm = normalize(channel);
    if channel_norm.is_empty() {
        return (left, right);
    }
    let left_norm = normalize(left);
    let right_norm = normalize(right);

    if !left_norm.is_empty() && (channel_norm.contains(&left_norm) || left_norm.contains(&channel_norm)) {
        return (left, right);
    }
    if !right_norm.is_empty() && (channel_norm.contains(&right_norm) || right_norm.contains(&channel_norm)) {
        return (right, left);
    }
    (left, right)
}

/// Kandidat query untuk pencarian otomatis saat hasil pertama gagal.
pub fn search_candidates(raw_title: &str, channel: Option<&str>) -> Vec<ParsedTrack> {
    let base = parse_title(raw_title, channel);
    let mut list = Vec::new();

    push_unique(&mut list, &base.artist, &base.track);

    if let Some(channel) = channel {
        let cleaned = clean_channel(channel);
        if !cleaned.is_empty() {
            push_unique(&mut list, &cleaned, &base.track);
        }
    }

    let primary = base
        .artist
        .split(|c| matches!(c, '&' | ',' | '/' | ';' | '+'))
        .next()
        .unwrap_or("")
        .trim();
    if !primary.is_empty() && primary != base.artist {
        push_unique(&mut list, primary, &base.track);
    }

    // Pencarian judul saja: LRCLIB tetap memberi skor berdasarkan durasi.
    push_unique(&mut list, "", &base.track);

    let without_groups = strip_parenthetical(&base.track);
    if !without_groups.is_empty() && without_groups != base.track {
        push_unique(&mut list, &base.artist, &without_groups);
    }

    // Judul mentah yang sudah dibersihkan sebagai pencarian bebas terakhir.
    push_unique(&mut list, "", &clean_title(raw_title));

    list
}

fn push_unique(list: &mut Vec<ParsedTrack>, artist: &str, track: &str) {
    let artist = tidy_piece(artist);
    let track = tidy_piece(track);
    if track.is_empty() {
        return;
    }
    let duplicate = list.iter().any(|candidate| {
        candidate.artist.eq_ignore_ascii_case(&artist)
            && candidate.track.eq_ignore_ascii_case(&track)
    });
    if !duplicate {
        list.push(ParsedTrack { artist, track });
    }
}

fn strip_parenthetical(text: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for ch in text.chars() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    tidy_piece(&out)
}

fn split_pair(text: &str) -> Option<(&str, &str)> {
    const SEPARATORS: [&str; 5] = [" - ", " – ", " — ", " | ", " // "];
    let mut best: Option<(usize, usize)> = None;

    for separator in SEPARATORS {
        if let Some(index) = text.find(separator) {
            if best.map(|(best_index, _)| index < best_index).unwrap_or(true) {
                best = Some((index, separator.len()));
            }
        }
    }

    let (index, length) = best?;
    let left = text[..index].trim();
    let right = text[index + length..].trim();
    if left.is_empty() || right.is_empty() {
        return None;
    }
    Some((left, right))
}

fn is_noise_group(group: &str) -> bool {
    let lower = group.to_lowercase();

    if ["feat.", "feat ", "ft.", "featuring", "prod.", "prod by"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        return true;
    }
    if lower.contains("remix") || lower.contains("acoustic") || lower.contains("live") {
        return false;
    }
    if ["music video", "m/v", "color coded", "sped up", "nightcore", "lyric video"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        return true;
    }

    lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .any(|token| {
            matches!(
                token,
                "official"
                    | "video"
                    | "audio"
                    | "lyrics"
                    | "lyric"
                    | "visualizer"
                    | "visualiser"
                    | "mv"
                    | "hd"
                    | "hq"
                    | "4k"
                    | "8k"
                    | "remaster"
                    | "remastered"
                    | "explicit"
                    | "1080p"
                    | "2160p"
                    | "720p"
            )
        })
}

fn strip_trailing_noise(text: &str) -> String {
    let mut value = text.trim().to_string();

    loop {
        let lower = value.to_lowercase();
        let mut changed = false;

        for phrase in [
            "official music video",
            "official lyric video",
            "official video",
            "official audio",
            "lyric video",
            "lyrics video",
            "music video",
            "official",
            "lyrics",
            "lyric",
            "audio",
        ] {
            if lower.ends_with(phrase) {
                let cut = value.len() - phrase.len();
                let prefix = tidy_piece(&value[..cut]);
                if !prefix.is_empty() {
                    value = prefix;
                    changed = true;
                    break;
                }
            }
        }

        if !changed {
            break;
        }
    }

    value
}

fn tidy_piece(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut previous_space = false;

    for ch in text.chars() {
        let ch = if ch == '\u{00a0}' { ' ' } else { ch };
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

    out.trim_matches(|c: char| {
        c.is_whitespace() || matches!(c, '-' | '|' | ':' | ',' | ';' | '·' | '•')
    })
    .to_string()
}

fn normalize(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artist_dash_song_with_noise() {
        let parsed = parse_title(
            "Rick Astley - Never Gonna Give You Up (Official Video)",
            Some("Rick Astley"),
        );
        assert_eq!(parsed.artist, "Rick Astley");
        assert_eq!(parsed.track, "Never Gonna Give You Up");
    }

    #[test]
    fn reversed_sides_using_channel() {
        let parsed = parse_title("Hello - Adele", Some("Adele - Topic"));
        assert_eq!(parsed.artist, "Adele");
        assert_eq!(parsed.track, "Hello");
    }

    #[test]
    fn title_only_uses_channel() {
        let parsed = parse_title("Some Song (Lyric Video)", Some("Cool Channel"));
        assert_eq!(parsed.artist, "Cool Channel");
        assert_eq!(parsed.track, "Some Song");
    }

    #[test]
    fn strips_featuring_group() {
        let parsed = parse_title("Song Name (feat. Guest)", Some("Artist"));
        assert_eq!(parsed.track, "Song Name");
        assert_eq!(parsed.artist, "Artist");
    }

    #[test]
    fn vevo_channel_cleanup() {
        assert_eq!(clean_channel("LadyGagaVEVO"), "LadyGaga");
        assert_eq!(clean_channel("Adele - Topic"), "Adele");
    }

    #[test]
    fn plain_pipe_noise_suffix() {
        let parsed = parse_title("Song Name | Official Video", Some("Channel"));
        assert_eq!(parsed.track, "Song Name");
    }

    #[test]
    fn search_candidates_are_diverse_and_deduplicated() {
        let candidates = search_candidates(
            "Rick Astley - Never Gonna Give You Up (Official Video)",
            Some("Rick Astley"),
        );

        assert!(candidates
            .iter()
            .any(|c| c.artist == "Rick Astley" && c.track == "Never Gonna Give You Up"));
        assert!(candidates
            .iter()
            .any(|c| c.artist.is_empty() && c.track == "Never Gonna Give You Up"));
        assert!(candidates.len() >= 3);

        let mut keys: Vec<String> = candidates
            .iter()
            .map(|c| format!("{}|{}", c.artist.to_lowercase(), c.track.to_lowercase()))
            .collect();
        let total = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(total, keys.len(), "kandidat duplikat: {keys:?}");
    }

    #[test]
    fn search_candidates_include_primary_artist() {
        let candidates = search_candidates("Song A - Alice & Bob", Some("Alice & Bob"));
        assert!(candidates
            .iter()
            .any(|c| c.artist == "Alice" && c.track == "Song A"));
    }
}
