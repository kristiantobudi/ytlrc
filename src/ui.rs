use std::{
    io::{self, Write},
    time::{Duration, Instant},
};

use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        cursor,
        event::{self, Event as CrosstermEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, LineGauge, Padding, Paragraph, Wrap},
    Frame, Terminal,
};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::{
    config,
    lyrics::{
        self,
        cache::Cache,
        lrc::{self, LyricLine},
        title::{self, ParsedTrack},
        FetchOutcome,
    },
    model::PlaybackState,
    server::PlaybackRef,
    theme::{self, Theme},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Synced,
    Plain,
}

#[derive(Debug)]
enum Event {
    Lyrics { key: String, outcome: FetchOutcome },
}

#[derive(Debug, Default)]
struct LyricsView {
    loading: bool,
    track: String,
    artist: String,
    synced: bool,
    lines: Vec<LyricLine>,
    offset_ms: i64,
    plain: Option<String>,
    status: String,
    error: Option<String>,
}

struct App {
    playback: PlaybackRef,
    cache: Cache,
    tx: UnboundedSender<Event>,
    rx: UnboundedReceiver<Event>,
    lyrics: LyricsView,
    query: Option<ParsedTrack>,
    last_key: Option<String>,
    raw_title: String,
    channel: Option<String>,
    offset: f64,
    mode: Mode,
    plain_scroll: usize,
    input: Option<String>,
    toast: Option<(String, Instant)>,
    ad: bool,
    waiting: bool,
    theme_index: usize,
    failures: u32,
    next_retry: Option<Instant>,
    manual_lookup: bool,
    /// Track key the current lyrics (or in-flight lookup) belong to.
    view_key: Option<String>,
    keys: config::Keybindings,
    /// When the current track started, used for the active-line pulse and
    /// the brief "new track" flash in the header.
    track_started: Instant,
}

impl App {
    fn new(playback: PlaybackRef, cache: Cache, theme_override: Option<usize>) -> Self {
        let (tx, rx) = unbounded_channel();
        let stored = config::load();
        let stored_theme = stored.theme.as_deref().and_then(theme::by_name);
        let theme_index = theme_override
            .or(stored_theme)
            .unwrap_or(0)
            .min(theme::THEMES.len() - 1);

        if theme_override.is_some() {
            config::save_theme(theme::THEMES[theme_index].name);
        }

        Self {
            playback,
            cache,
            tx,
            rx,
            lyrics: LyricsView::default(),
            query: None,
            last_key: None,
            raw_title: String::new(),
            channel: None,
            offset: 0.0,
            mode: Mode::Synced,
            plain_scroll: 0,
            input: None,
            toast: None,
            ad: false,
            waiting: true,
            theme_index,
            failures: 0,
            next_retry: None,
            manual_lookup: false,
            view_key: None,
            keys: stored.keybindings,
            track_started: Instant::now(),
        }
    }

    fn theme(&self) -> Theme {
        let theme = theme::THEMES[self.theme_index.min(theme::THEMES.len() - 1)];
        if theme::no_color() {
            theme.no_color()
        } else {
            theme
        }
    }

    fn set_theme(&mut self, index: usize) {
        let index = index % theme::THEMES.len();
        self.theme_index = index;
        let name = theme::THEMES[index].name;
        config::save_theme(name);
        self.toast = Some((
            format!("Theme {}/{}: {name}", index + 1, theme::THEMES.len()),
            Instant::now(),
        ));
    }

    /// Remember the current offset for the active track so it is restored
    /// the next time the same song plays.
    fn persist_offset(&self) {
        if let Some(key) = &self.last_key {
            config::save_offset(key, self.offset);
        }
    }

    fn poll_playback(&mut self) {
        let snapshot = self.playback.lock().unwrap().clone();
        let Some(state) = snapshot else {
            self.waiting = true;
            return;
        };

        self.waiting = state.received_at.elapsed().as_secs_f64() > 20.0;

        if state.is_ad {
            self.ad = true;
            return;
        }
        self.ad = false;

        let key = state.track_key();
        let same_track = self.last_key.as_deref() == Some(key.as_str());

        // Same song and its lyrics are already on screen (or being fetched).
        if same_track && self.view_key.as_deref() == Some(key.as_str()) {
            return;
        }

        // Either the song changed, or the displayed lyrics no longer belong to
        // the current track (a stale lookup, a lost update, a live stream that
        // switched songs). In both cases: start a fresh lookup right away.
        self.begin_track(&state, key);
    }

    fn begin_track(&mut self, state: &PlaybackState, key: String) {
        self.last_key = Some(key.clone());
        self.offset = config::load_offset(&key).unwrap_or(0.0);
        self.plain_scroll = 0;
        self.mode = Mode::Synced;
        self.failures = 0;
        self.next_retry = None;
        self.manual_lookup = false;
        self.raw_title = state.title.clone();
        self.channel = state.channel.clone();
        self.track_started = Instant::now();

        let mut parsed = title::parse_title(&state.title, state.channel.as_deref());
        // YouTube Music exposes a clean artist name through mediaSession.
        if state.source.contains("music") {
            if let Some(artist) = state
                .artist
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                parsed.artist = artist.to_string();
            }
        }

        self.query = Some(parsed.clone());
        self.lyrics = LyricsView {
            loading: true,
            track: parsed.track.clone(),
            artist: parsed.artist.clone(),
            status: "searching lyrics…".to_string(),
            ..Default::default()
        };

        self.start_fetch(
            key,
            parsed,
            state.video_id.clone(),
            state.duration,
            true,
            true,
            state.title.clone(),
            state.channel.clone(),
        );
    }

    fn maybe_retry(&mut self) {
        if self.manual_lookup || self.ad || self.waiting {
            return;
        }
        let Some(retry_at) = self.next_retry else {
            return;
        };
        if Instant::now() < retry_at {
            return;
        }
        self.next_retry = None;

        let Some(key) = self.last_key.clone() else {
            return;
        };
        let Some(parsed) = self.query.clone() else {
            return;
        };
        let Some(state) = self.playback.lock().unwrap().clone() else {
            return;
        };

        self.lyrics.loading = true;
        self.lyrics.error = None;
        self.lyrics.status = "retrying…".to_string();

        self.start_fetch(
            key,
            parsed,
            state.video_id.clone(),
            state.duration,
            false,
            true,
            self.raw_title.clone(),
            self.channel.clone(),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn start_fetch(
        &mut self,
        key: String,
        parsed: ParsedTrack,
        video_id: String,
        duration: f64,
        use_cache: bool,
        deep: bool,
        raw_title: String,
        channel: Option<String>,
    ) {
        let cache = self.cache.clone();
        let tx = self.tx.clone();
        self.view_key = Some(key.clone());

        tokio::spawn(async move {
            let request = lyrics::Lookup {
                video_id: &video_id,
                parsed: &parsed,
                duration,
                use_cache,
                deep,
                raw_title: &raw_title,
                channel: channel.as_deref(),
            };
            let outcome = lyrics::fetch(&cache, &request).await;
            let _ = tx.send(Event::Lyrics { key, outcome });
        });
    }

    fn refetch(&mut self, use_cache: bool) {
        let Some(parsed) = self.query.clone() else {
            return;
        };
        let Some(state) = self.playback.lock().unwrap().clone() else {
            return;
        };
        let Some(key) = self.last_key.clone() else {
            return;
        };

        if !use_cache {
            self.cache
                .invalidate(&Cache::key(&state.video_id, &parsed.artist, &parsed.track));
        }

        self.failures = 0;
        self.next_retry = None;
        self.lyrics.loading = true;
        self.lyrics.error = None;
        self.lyrics.status = "searching lyrics…".to_string();
        self.start_fetch(
            key,
            parsed,
            state.video_id.clone(),
            state.duration,
            use_cache,
            true,
            self.raw_title.clone(),
            self.channel.clone(),
        );
        self.toast = Some(("Refetching lyrics…".to_string(), Instant::now()));
    }

    fn submit_manual(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }

        let Some(key) = self.last_key.clone() else {
            self.toast = Some(("No track detected yet.".to_string(), Instant::now()));
            return;
        };

        let mut parsed = title::parse_title(text, None);
        if !text.contains(" - ") && !text.contains(" – ") && !text.contains(" — ") {
            // Without a separator, treat the input as a free-text title search.
            parsed.artist.clear();
        }
        self.query = Some(parsed.clone());
        self.manual_lookup = true;
        self.failures = 0;
        self.next_retry = None;

        let video_id = self
            .playback
            .lock()
            .unwrap()
            .as_ref()
            .map(|state| state.video_id.clone())
            .unwrap_or_default();
        let duration = self
            .playback
            .lock()
            .unwrap()
            .as_ref()
            .map(|state| state.duration)
            .unwrap_or(0.0);

        self.cache
            .invalidate(&Cache::key(&video_id, &parsed.artist, &parsed.track));
        self.lyrics = LyricsView {
            loading: true,
            track: parsed.track.clone(),
            artist: parsed.artist.clone(),
            status: "searching lyrics…".to_string(),
            ..Default::default()
        };
        self.start_fetch(
            key,
            parsed,
            video_id,
            duration,
            false,
            true,
            text.to_string(),
            None,
        );
    }

    fn on_event(&mut self, event: Event) {
        let Event::Lyrics { key, outcome } = event;
        if self.last_key.as_deref() != Some(key.as_str()) {
            return;
        }

        match outcome {
            FetchOutcome::Found(found) => {
                let status = match (found.synced, found.cached) {
                    (true, true) => "synced • cache",
                    (true, false) => "synced",
                    (false, true) => "plain • cache",
                    (false, false) => "plain",
                }
                .to_string();

                self.failures = 0;
                self.next_retry = None;
                self.mode = if found.synced {
                    Mode::Synced
                } else {
                    Mode::Plain
                };
                self.plain_scroll = 0;
                self.lyrics = LyricsView {
                    loading: false,
                    track: found.track,
                    artist: found.artist,
                    synced: found.synced,
                    lines: found.lines,
                    offset_ms: found.offset_ms,
                    plain: found.plain,
                    status,
                    error: None,
                };
            }
            FetchOutcome::NotFound(message) => {
                self.schedule_retry();
                let (track, artist) = self.query_labels();
                self.lyrics = LyricsView {
                    loading: false,
                    track,
                    artist,
                    status: "not found".to_string(),
                    error: Some(message),
                    ..Default::default()
                };
            }
            FetchOutcome::Error(message) => {
                self.schedule_retry();
                let (track, artist) = self.query_labels();
                self.lyrics = LyricsView {
                    loading: false,
                    track,
                    artist,
                    status: "error".to_string(),
                    error: Some(message),
                    ..Default::default()
                };
            }
        }
    }

    fn schedule_retry(&mut self) {
        if self.manual_lookup {
            return;
        }
        let delay = retry_delay(self.failures);
        self.next_retry = Some(Instant::now() + Duration::from_secs(delay));
        self.failures = self.failures.saturating_add(1);
    }

    fn query_labels(&self) -> (String, String) {
        self.query
            .as_ref()
            .map(|query| (query.track.clone(), query.artist.clone()))
            .unwrap_or_default()
    }

    /// Returns `true` when the app should exit.
    fn on_key(&mut self, key: KeyEvent) -> bool {
        if self.input.is_some() {
            let mut close = false;
            let mut submit = None;

            if let Some(buffer) = self.input.as_mut() {
                match key.code {
                    KeyCode::Esc => close = true,
                    KeyCode::Enter => {
                        submit = Some(buffer.clone());
                        close = true;
                    }
                    KeyCode::Backspace => {
                        buffer.pop();
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        buffer.clear();
                    }
                    KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        buffer.push(ch);
                    }
                    _ => {}
                }
            }

            if close {
                self.input = None;
            }
            if let Some(text) = submit {
                self.submit_manual(&text);
            }
            return false;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return true,
            KeyCode::Char(ch) if ch == self.keys.offset_up || ch == '=' => {
                self.offset += 0.25;
                self.persist_offset();
            }
            KeyCode::Char(ch) if ch == self.keys.offset_down || ch == '_' => {
                self.offset = (self.offset - 0.25).max(-30.0);
                self.persist_offset();
            }
            KeyCode::Char(ch) if ch == self.keys.offset_reset => {
                self.offset = 0.0;
                self.persist_offset();
            }
            KeyCode::Char(ch) if ch == self.keys.theme_next => self.set_theme(self.theme_index + 1),
            KeyCode::Char(ch @ '1'..='5') => {
                if let Some(index) = ch.to_digit(10) {
                    self.set_theme(index as usize - 1);
                }
            }
            KeyCode::Char(ch) if ch == self.keys.toggle_mode => {
                if self.lyrics.synced {
                    self.mode = match self.mode {
                        Mode::Synced => Mode::Plain,
                        Mode::Plain => Mode::Synced,
                    };
                }
            }
            KeyCode::Char(ch) if ch == self.keys.refetch => self.refetch(false),
            KeyCode::Char(ch) if ch == self.keys.manual_search => {
                let seed = if self.lyrics.artist.is_empty() {
                    String::new()
                } else {
                    format!("{} - {}", self.lyrics.artist, self.lyrics.track)
                };
                self.input = Some(seed);
            }
            KeyCode::Up => self.plain_scroll = self.plain_scroll.saturating_sub(1),
            KeyCode::Down => self.plain_scroll += 1,
            KeyCode::PageUp => self.plain_scroll = self.plain_scroll.saturating_sub(10),
            KeyCode::PageDown => self.plain_scroll += 10,
            KeyCode::Home => self.plain_scroll = 0,
            _ => {}
        }

        false
    }

    fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        if area.height < 8 || area.width < 20 {
            frame.render_widget(
                Paragraph::new("Terminal too small. Please resize the window."),
                area,
            );
            return;
        }

        let state = self.playback.lock().unwrap().clone();
        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(3),
        ])
        .split(area);

        self.render_header(frame, chunks[0], state.as_ref());
        self.render_lyrics(frame, chunks[1], state.as_ref());
        self.render_footer(frame, chunks[2], state.as_ref());
    }

    fn status_text(&self) -> String {
        if self.ad {
            return "AD".to_string();
        }
        if self.waiting {
            return "WAITING".to_string();
        }

        let mut status = self.lyrics.status.to_uppercase();
        if let Some(retry_at) = self.next_retry {
            let seconds = retry_at.saturating_duration_since(Instant::now()).as_secs();
            status = format!("{status} · RETRY {seconds}s");
        }
        status
    }

    fn status_color(&self) -> Color {
        let theme = self.theme();
        if self.ad || self.next_retry.is_some() {
            return theme.warn;
        }
        if self.waiting {
            return theme.hint;
        }
        if self.lyrics.error.is_some() {
            return theme.err;
        }
        if self.lyrics.synced || self.lyrics.loading {
            theme.ok
        } else {
            theme.text
        }
    }

    fn render_header(&self, frame: &mut Frame, area: Rect, state: Option<&PlaybackState>) {
        let theme = self.theme();
        let (icon, title) = if self.ad {
            ("▶", "Ad".to_string())
        } else if !self.lyrics.track.is_empty() {
            let icon = if state.map(|st| st.paused).unwrap_or(false) {
                "❚❚"
            } else {
                "▶"
            };
            (
                icon,
                format!("{} — {}", self.lyrics.artist, self.lyrics.track),
            )
        } else if let Some(state) = state {
            ("▶", state.title.clone())
        } else {
            ("·", "Waiting for YouTube…".to_string())
        };

        let status = self.status_text();
        let status_color = self.status_color();
        let is_new_track = !self.lyrics.track.is_empty()
            && self.track_started.elapsed() < Duration::from_millis(1500);

        let left = if is_new_track {
            format!("{icon} {title}  ✦ NEW")
        } else {
            format!("{icon} {title}")
        };
        let pad = area
            .width
            .saturating_sub(left.chars().count() as u16 + status.chars().count() as u16 + 2)
            as usize;

        let mut subtitle = String::new();
        if let Some(state) = state {
            subtitle.push_str(&format!("{}  •  {}", state.source, state.video_id));
            if state.paused {
                subtitle.push_str("  •  paused");
            }
        } else {
            subtitle.push_str("no data from the browser extension yet");
        }
        if self.waiting && state.is_some() {
            subtitle.push_str("  •  no update for >20s");
        }
        subtitle.push_str(&format!("  •  theme {}", theme.name));

        let title_text = format!("{icon} {title}");
        let left_spans = if is_new_track {
            vec![
                Span::styled(
                    title_text,
                    Style::new().fg(theme.title).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "  ✦ NEW",
                    Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
                ),
            ]
        } else {
            vec![Span::styled(
                title_text,
                Style::new().fg(theme.title).add_modifier(Modifier::BOLD),
            )]
        };

        let header = Paragraph::new(vec![
            Line::from(
                left_spans
                    .into_iter()
                    .chain([
                        Span::raw(" ".repeat(pad)),
                        Span::styled(
                            status,
                            Style::new().fg(status_color).add_modifier(Modifier::BOLD),
                        ),
                    ])
                    .collect::<Vec<_>>(),
            ),
            Line::from(Span::styled(subtitle, Style::new().fg(theme.hint))),
        ])
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::new().fg(theme.border)),
        );

        frame.render_widget(header, area);
    }

    fn render_lyrics(&self, frame: &mut Frame, area: Rect, state: Option<&PlaybackState>) {
        let theme = self.theme();
        let block = Block::default().padding(Padding::new(2, 2, 0, 0));

        if self.ad {
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "⏸  Advertisement playing — lyrics paused.",
                        Style::new().fg(theme.warn),
                    )),
                ])
                .block(block),
                area,
            );
            return;
        }

        if self.waiting {
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "Menunggu YouTube…",
                        Style::new().fg(theme.title).add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Open youtube.com or music.youtube.com and make sure the extension",
                        Style::new().fg(theme.hint),
                    )),
                    Line::from(Span::styled(
                        "\"ytlrc bridge\" is enabled and the daemon is running on port 8765.",
                        Style::new().fg(theme.hint),
                    )),
                ])
                .block(block),
                area,
            );
            return;
        }

        if self.lyrics.loading {
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        format!(
                            "Searching lyrics: {} — {}",
                            self.lyrics.artist, self.lyrics.track
                        ),
                        Style::new().fg(theme.accent),
                    )),
                ])
                .block(block),
                area,
            );
            return;
        }

        if let Some(error) = &self.lyrics.error {
            let retry_hint = match self.next_retry {
                Some(retry_at) => {
                    let seconds = retry_at.saturating_duration_since(Instant::now()).as_secs();
                    format!("Retrying automatically in {seconds}s…  (r = now, m = manual)")
                }
                None => "Press r to retry or m to search manually.".to_string(),
            };

            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "Lyrics not found.",
                        Style::new().fg(theme.warn).add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(error.clone(), Style::new().fg(theme.text))),
                    Line::from(""),
                    Line::from(Span::styled(retry_hint, Style::new().fg(theme.hint))),
                ])
                .wrap(Wrap { trim: true })
                .block(block),
                area,
            );
            return;
        }

        if self.mode == Mode::Synced && !self.lyrics.lines.is_empty() {
            self.render_synced(frame, area, state, block);
            return;
        }

        let lines = self.plain_lines();
        if lines.is_empty() {
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "No lyrics available.",
                        Style::new().fg(theme.text),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Press r to retry or m to search manually.",
                        Style::new().fg(theme.hint),
                    )),
                ])
                .block(block),
                area,
            );
            return;
        }

        let height = area.height as usize;
        let start = self.plain_scroll.min(lines.len().saturating_sub(height));
        let visible: Vec<Line> = lines
            .iter()
            .skip(start)
            .take(height)
            .map(|text| Line::from(Span::styled(text.clone(), Style::new().fg(theme.text))))
            .collect();

        frame.render_widget(Paragraph::new(visible).block(block), area);
    }

    fn render_synced(
        &self,
        frame: &mut Frame,
        area: Rect,
        state: Option<&PlaybackState>,
        block: Block,
    ) {
        let theme = self.theme();
        let position = state.map(|st| st.position_now()).unwrap_or(0.0);
        let pos_ms = (position * 1000.0).round() as i64
            + (self.offset * 1000.0).round() as i64
            + self.lyrics.offset_ms;

        let active = lrc::active_index(&self.lyrics.lines, pos_ms);
        let height = area.height as usize;
        let anchor = (height * 2 / 5).max(1);
        let start = match active {
            Some(index) => {
                let mut start = index.saturating_sub(anchor);
                if start + height > self.lyrics.lines.len() {
                    start = self.lyrics.lines.len().saturating_sub(height);
                }
                start
            }
            None => 0,
        };

        let lines: Vec<Line> = self
            .lyrics
            .lines
            .iter()
            .enumerate()
            .skip(start)
            .take(height)
            .map(|(index, line)| {
                let is_active = Some(index) == active;
                let is_past = active.map(|active| index < active).unwrap_or(false);
                let prefix = if is_active { "▶ " } else { "  " };

                if is_active && !line.words.is_empty() && !theme::reduce_motion() {
                    return self.render_karaoke_line(line, pos_ms, prefix, theme);
                }

                let text = if line.text.trim().is_empty() {
                    "♪".to_string()
                } else {
                    line.text.clone()
                };
                let style = if is_active {
                    Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)
                } else if is_past {
                    Style::new().fg(theme.past)
                } else {
                    Style::new().fg(theme.text)
                };
                Line::from(Span::styled(format!("{prefix}{text}"), style))
            })
            .collect();

        frame.render_widget(Paragraph::new(lines).block(block), area);
    }

    /// Renders the active line word by word: words already sung are in the
    /// accent color, the word currently being sung is bold + underlined, and
    /// words not yet reached fall back to the normal text color. Falls back
    /// to whole-line highlighting when the line carries no word timing.
    fn render_karaoke_line<'a>(
        &self,
        line: &'a LyricLine,
        pos_ms: i64,
        prefix: &'static str,
        theme: Theme,
    ) -> Line<'a> {
        let mut spans = vec![Span::styled(prefix, Style::new().fg(theme.accent))];
        for (index, word) in line.words.iter().enumerate() {
            if index > 0 {
                spans.push(Span::raw(" "));
            }
            let next_time = line
                .words
                .get(index + 1)
                .map(|next| next.time_ms)
                .unwrap_or(i64::MAX);
            let style = if pos_ms >= word.time_ms && pos_ms < next_time {
                Style::new()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else if pos_ms >= next_time {
                Style::new().fg(theme.accent)
            } else {
                Style::new().fg(theme.text)
            };
            spans.push(Span::styled(word.text.as_str(), style));
        }
        Line::from(spans)
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect, state: Option<&PlaybackState>) {
        let theme = self.theme();
        let position = state.map(|st| st.position_now()).unwrap_or(0.0);
        let duration = state.map(|st| st.duration).unwrap_or(0.0);
        let ratio = if duration > 0.0 {
            (position / duration).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let label = format!(
            "{} / {}   offset {:+.2}s",
            format_time(position),
            format_time(duration),
            self.offset
        );
        let gauge = LineGauge::default()
            .ratio(ratio)
            .label(Line::from(Span::styled(label, Style::new().fg(theme.text))))
            .filled_style(Style::new().fg(theme.accent))
            .unfilled_style(Style::new().fg(theme.gauge_track));

        let rows = Layout::vertical([Constraint::Length(1), Constraint::Length(2)]).split(area);
        frame.render_widget(gauge, rows[0]);

        if let Some(buffer) = &self.input {
            let text = format!("Search lyrics (Artist - Title): {buffer}█");
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(text, Style::new().fg(theme.warn)))),
                rows[1],
            );
            return;
        }

        let hint = if let Some((message, at)) = &self.toast {
            if at.elapsed() < Duration::from_secs(4) {
                message.clone()
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let keys = if hint.is_empty() {
            format!(
                "q quit · {}/{} offset · {} synced/plain · {} theme ({}) · {} retry · {} search · ↑↓ scroll",
                self.keys.offset_up,
                self.keys.offset_down,
                self.keys.toggle_mode,
                self.keys.theme_next,
                theme.name,
                self.keys.refetch,
                self.keys.manual_search,
            )
        } else {
            hint
        };

        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(keys, Style::new().fg(theme.hint)))),
            rows[1],
        );
    }

    fn plain_lines(&self) -> Vec<String> {
        if let Some(plain) = &self.lyrics.plain {
            return plain.lines().map(|line| line.to_string()).collect();
        }
        if !self.lyrics.lines.is_empty() {
            return self
                .lyrics
                .lines
                .iter()
                .map(|line| line.text.clone())
                .collect();
        }
        Vec::new()
    }
}

/// Delay before the next automatic lookup attempt, in seconds.
fn retry_delay(failures: u32) -> u64 {
    match failures {
        0 => 8,
        1 => 25,
        2 => 90,
        3 => 240,
        _ => 300,
    }
}

fn format_time(seconds: f64) -> String {
    if !seconds.is_finite() || seconds <= 0.0 {
        return "00:00".to_string();
    }
    let total = seconds.round() as u64;
    format!("{:02}:{:02}", total / 60, total % 60)
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen, cursor::Show);
    }
}

pub async fn run(
    playback: PlaybackRef,
    cache: Cache,
    theme_override: Option<usize>,
) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    let _guard = TerminalGuard;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut app = App::new(playback, cache, theme_override);

    loop {
        app.poll_playback();
        app.maybe_retry();
        while let Ok(event) = app.rx.try_recv() {
            app.on_event(event);
        }

        terminal.draw(|frame| app.render(frame))?;

        if event::poll(Duration::from_millis(60))? {
            if let CrosstermEvent::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press && app.on_key(key) {
                    break;
                }
            }
        }
    }

    Ok(())
}

/// Headless mode: print the active lyric line to stdout. Useful for debugging
/// and for scripts or stream overlays.
pub async fn run_print(playback: PlaybackRef, cache: Cache) -> anyhow::Result<()> {
    let mut last_key: Option<String> = None;
    let mut lyrics: Option<lyrics::FoundLyrics> = None;
    let mut last_line: Option<String> = None;
    let mut failures = 0u32;
    let mut next_retry: Option<Instant> = None;

    loop {
        tokio::time::sleep(Duration::from_millis(150)).await;

        let state = playback.lock().unwrap().clone();
        let Some(state) = state else { continue };
        if state.is_ad {
            continue;
        }

        let key = state.track_key();
        if last_key.as_deref() != Some(key.as_str()) {
            last_key = Some(key);
            last_line = None;
            failures = 0;
            next_retry = None;

            let parsed = print_parse(&state);
            println!("\n▶ {}", state.title);
            println!("  {} — {}", parsed.artist, parsed.track);

            let outcome = print_fetch(&cache, &state, &parsed, true).await;
            match outcome {
                FetchOutcome::Found(found) => {
                    print_found(&found);
                    lyrics = Some(found);
                }
                FetchOutcome::NotFound(message) => {
                    println!("  (lyrics not found: {message})");
                    lyrics = None;
                    failures += 1;
                    next_retry = Some(retry_at(failures));
                    print_retry(next_retry);
                }
                FetchOutcome::Error(message) => {
                    println!("  (error: {message})");
                    lyrics = None;
                    failures += 1;
                    next_retry = Some(retry_at(failures));
                    print_retry(next_retry);
                }
            }
            println!();
            continue;
        }

        if lyrics.is_none() {
            match next_retry {
                Some(at) if Instant::now() >= at => {
                    next_retry = None;
                    let parsed = print_parse(&state);
                    println!("  retrying…");

                    let outcome = print_fetch(&cache, &state, &parsed, false).await;
                    match outcome {
                        FetchOutcome::Found(found) => {
                            print_found(&found);
                            lyrics = Some(found);
                            failures = 0;
                        }
                        FetchOutcome::NotFound(message) => {
                            println!("  (lyrics not found: {message})");
                            failures += 1;
                            next_retry = Some(retry_at(failures));
                            print_retry(next_retry);
                        }
                        FetchOutcome::Error(message) => {
                            println!("  (error: {message})");
                            failures += 1;
                            next_retry = Some(retry_at(failures));
                            print_retry(next_retry);
                        }
                    }
                    println!();
                }
                _ => {}
            }

            if lyrics.is_none() {
                continue;
            }
        }

        let Some(found) = &lyrics else { continue };
        if !found.synced {
            continue;
        }

        let pos_ms = (state.position_now() * 1000.0).round() as i64 + found.offset_ms;
        let Some(index) = lrc::active_index(&found.lines, pos_ms) else {
            continue;
        };
        let text = if found.lines[index].text.trim().is_empty() {
            "♪".to_string()
        } else {
            found.lines[index].text.clone()
        };
        if last_line.as_deref() != Some(text.as_str()) {
            print!("\r\x1b[K  ♪ {text}");
            let _ = io::stdout().flush();
            last_line = Some(text);
        }
    }
}

fn print_parse(state: &PlaybackState) -> ParsedTrack {
    let mut parsed = title::parse_title(&state.title, state.channel.as_deref());
    if state.source.contains("music") {
        if let Some(artist) = state
            .artist
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            parsed.artist = artist.to_string();
        }
    }
    parsed
}

async fn print_fetch(
    cache: &Cache,
    state: &PlaybackState,
    parsed: &ParsedTrack,
    use_cache: bool,
) -> FetchOutcome {
    let request = lyrics::Lookup {
        video_id: &state.video_id,
        parsed,
        duration: state.duration,
        use_cache,
        deep: true,
        raw_title: &state.title,
        channel: state.channel.as_deref(),
    };
    lyrics::fetch(cache, &request).await
}

fn print_found(found: &lyrics::FoundLyrics) {
    println!(
        "  [{}]{}",
        if found.synced { "synced" } else { "plain" },
        if found.cached { " (cache)" } else { "" }
    );
    if !found.synced {
        if let Some(plain) = &found.plain {
            println!();
            println!("{plain}");
        }
    }
}

fn retry_at(failures: u32) -> Instant {
    Instant::now() + Duration::from_secs(retry_delay(failures.saturating_sub(1)))
}

fn print_retry(next_retry: Option<Instant>) {
    if let Some(at) = next_retry {
        let seconds = at.saturating_duration_since(Instant::now()).as_secs();
        println!("  (retrying automatically in {seconds}s)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_backoff_grows_and_caps() {
        assert_eq!(retry_delay(0), 8);
        assert_eq!(retry_delay(1), 25);
        assert_eq!(retry_delay(2), 90);
        assert_eq!(retry_delay(3), 240);
        assert_eq!(retry_delay(4), 300);
        assert_eq!(retry_delay(99), 300);
    }
}
