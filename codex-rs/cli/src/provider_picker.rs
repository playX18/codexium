use codex_models_dev::ModelsDevProvider;
use codex_utils_fuzzy_match::fuzzy_match;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use crossterm::event::{self};
use crossterm::execute;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::prelude::*;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use std::collections::HashMap;
use std::io;

const FOOTER_HINT: &str = "↑/↓ j/k navigate • Enter select • Esc/q cancel";

fn effective_viewport_rows(viewport_rows: usize, filtered_len: usize) -> usize {
    if filtered_len > viewport_rows {
        viewport_rows.saturating_sub(1).max(1)
    } else {
        viewport_rows.max(1)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProviderEntry {
    id: String,
    name: String,
}

#[derive(Debug)]
struct ProviderPicker {
    entries: Vec<ProviderEntry>,
    filtered: Vec<usize>,
    selected: usize,
    scroll_top: usize,
    query: String,
    done: bool,
    cancelled: bool,
    selection: Option<String>,
}

impl ProviderPicker {
    fn new(providers: &HashMap<String, ModelsDevProvider>) -> Self {
        let mut entries: Vec<ProviderEntry> = providers
            .iter()
            .map(|(id, provider)| ProviderEntry {
                id: id.clone(),
                name: provider.name.clone(),
            })
            .collect();
        entries.sort_by(|left, right| left.id.cmp(&right.id));
        let filtered: Vec<usize> = (0..entries.len()).collect();
        Self {
            entries,
            filtered,
            selected: 0,
            scroll_top: 0,
            query: String::new(),
            done: false,
            cancelled: false,
            selection: None,
        }
    }

    fn apply_filter(&mut self) {
        self.filtered = filter_provider_indices(&self.entries, &self.query);
        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len().saturating_sub(1);
        }
        self.scroll_top = 0;
    }

    fn ensure_selected_visible(&mut self, viewport_rows: usize) {
        if self.filtered.is_empty() {
            self.scroll_top = 0;
            return;
        }
        if self.selected < self.scroll_top {
            self.scroll_top = self.selected;
        } else if self.selected >= self.scroll_top.saturating_add(viewport_rows) {
            self.scroll_top = self
                .selected
                .saturating_add(1)
                .saturating_sub(viewport_rows);
        }
    }

    fn move_selection(&mut self, delta: isize, viewport_rows: usize) {
        if self.filtered.is_empty() {
            return;
        }
        let len = self.filtered.len() as isize;
        let next = (self.selected as isize + delta).rem_euclid(len);
        self.selected = next as usize;
        self.ensure_selected_visible(viewport_rows);
    }

    fn page(&mut self, direction: isize, viewport_rows: usize) {
        if viewport_rows == 0 {
            return;
        }
        self.move_selection(direction * viewport_rows as isize, viewport_rows);
    }

    fn filtered_len(&self) -> usize {
        self.filtered.len()
    }

    fn handle_key(&mut self, key: KeyEvent, viewport_rows: usize) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cancel();
            }
            KeyCode::Esc | KeyCode::Char('q') => self.cancel(),
            KeyCode::Enter => self.accept_selection(),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1, viewport_rows),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1, viewport_rows),
            KeyCode::PageUp => self.page(-1, viewport_rows),
            KeyCode::PageDown => self.page(1, viewport_rows),
            KeyCode::Home => {
                self.selected = 0;
                self.ensure_selected_visible(viewport_rows);
            }
            KeyCode::End => {
                self.selected = self.filtered.len().saturating_sub(1);
                self.ensure_selected_visible(viewport_rows);
            }
            KeyCode::Backspace => {
                self.query.pop();
                self.apply_filter();
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.query.push(ch);
                self.apply_filter();
            }
            _ => {}
        }
    }

    fn accept_selection(&mut self) {
        let Some(&entry_index) = self.filtered.get(self.selected) else {
            return;
        };
        self.selection = Some(self.entries[entry_index].id.clone());
        self.done = true;
    }

    fn cancel(&mut self) {
        self.cancelled = true;
        self.done = true;
    }

    fn render(&self, area: Rect, buf: &mut Buffer, viewport_rows: usize) {
        let block = Block::default()
            .title(" Select provider ")
            .borders(Borders::ALL);
        let inner = block.inner(area);
        block.render(area, buf);

        let [search_area, list_area, footer_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .areas(inner);

        let search_line = if self.query.is_empty() {
            Line::from(vec![
                "Search: ".dim(),
                "type to filter providers".dim().italic(),
            ])
        } else {
            Line::from(vec![
                "Search: ".dim(),
                self.query.clone().into(),
                format!("  ({} matches)", self.filtered.len()).dim(),
            ])
        };
        Paragraph::new(search_line).render(search_area, buf);

        if self.filtered.is_empty() {
            Paragraph::new(Line::from("No providers match your search.").dim())
                .render(list_area, buf);
        } else {
            let item_rows = effective_viewport_rows(viewport_rows, self.filtered.len());
            let visible_end = (self.scroll_top + item_rows).min(self.filtered.len());
            for (row, &entry_index) in self
                .filtered
                .iter()
                .enumerate()
                .skip(self.scroll_top)
                .take(item_rows)
            {
                let entry = &self.entries[entry_index];
                let selected = row + self.scroll_top == self.selected;
                let line = render_provider_row(entry, selected, &self.query);
                let y = list_area.y + (row - self.scroll_top) as u16;
                if y < list_area.bottom() {
                    line.render(Rect::new(list_area.x, y, list_area.width, 1), buf);
                }
            }
            if self.filtered.len() > viewport_rows {
                let indicator = format!(
                    "  {}–{} of {}",
                    self.scroll_top + 1,
                    visible_end,
                    self.filtered.len()
                );
                Paragraph::new(Line::from(indicator.dim())).render(
                    Rect::new(
                        list_area.x,
                        list_area.bottom().saturating_sub(1),
                        list_area.width,
                        1,
                    ),
                    buf,
                );
            }
        }

        Paragraph::new(Line::from(FOOTER_HINT.dim())).render(footer_area, buf);
    }
}

fn render_provider_row(entry: &ProviderEntry, selected: bool, query: &str) -> Line<'static> {
    let base_style = if selected {
        Style::default().bg(Color::Cyan).fg(Color::Black)
    } else {
        Style::default()
    };
    let id_spans = highlight_fuzzy_match(&entry.id, query, base_style, Style::default().bold());
    let name_spans = highlight_fuzzy_match(&entry.name, query, base_style, Style::default().bold());
    let mut spans = vec![Span::raw("  ")];
    spans.extend(id_spans);
    spans.push(Span::styled("  ", base_style));
    spans.extend(name_spans);
    Line::from(spans)
}

fn highlight_fuzzy_match(
    text: &str,
    query: &str,
    base_style: Style,
    match_style: Style,
) -> Vec<Span<'static>> {
    if query.is_empty() {
        return vec![Span::styled(text.to_string(), base_style)];
    }
    let Some((indices, _score)) = fuzzy_match(text, query) else {
        return vec![Span::styled(text.to_string(), base_style.dim())];
    };
    let match_set: std::collections::HashSet<usize> = indices.into_iter().collect();
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut current_style = base_style;
    for (idx, ch) in text.chars().enumerate() {
        let style = if match_set.contains(&idx) {
            base_style.patch(match_style)
        } else {
            base_style
        };
        if style != current_style && !current.is_empty() {
            spans.push(Span::styled(std::mem::take(&mut current), current_style));
            current_style = style;
        }
        current.push(ch);
    }
    if !current.is_empty() {
        spans.push(Span::styled(current, current_style));
    }
    spans
}

pub(crate) fn filter_provider_indices(entries: &[ProviderEntry], query: &str) -> Vec<usize> {
    if query.is_empty() {
        return (0..entries.len()).collect();
    }
    let mut matches: Vec<(usize, i32)> = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            provider_entry_match_score(entry, query).map(|score| (index, score))
        })
        .collect();
    matches.sort_by(|(index_a, score_a), (index_b, score_b)| {
        score_a
            .cmp(score_b)
            .then_with(|| entries[*index_a].id.cmp(&entries[*index_b].id))
    });
    matches.into_iter().map(|(index, _)| index).collect()
}

pub(crate) fn provider_entry_match_score(entry: &ProviderEntry, query: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(i32::MAX);
    }
    let id_score = fuzzy_match(&entry.id, query);
    let name_score = fuzzy_match(&entry.name, query);
    match (id_score, name_score) {
        (Some((_, score_a)), Some((_, score_b))) => Some(score_a.min(score_b)),
        (Some((_, score)), None) => Some(score),
        (None, Some((_, score))) => Some(score),
        (None, None) => {
            let query_lower = query.to_ascii_lowercase();
            if entry.id.to_ascii_lowercase().contains(&query_lower)
                || entry.name.to_ascii_lowercase().contains(&query_lower)
            {
                Some(0)
            } else {
                None
            }
        }
    }
}

pub(crate) fn pick_provider_id(
    providers: &HashMap<String, ModelsDevProvider>,
) -> io::Result<String> {
    if providers.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no providers available",
        ));
    }

    let mut picker = ProviderPicker::new(providers);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = loop {
        let viewport_rows = terminal.size()?.height.saturating_sub(6) as usize;
        terminal.draw(|frame| {
            picker.render(frame.area(), frame.buffer_mut(), viewport_rows.max(1));
        })?;

        if picker.done {
            break if picker.cancelled {
                Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "provider selection cancelled",
                ))
            } else {
                picker.selection.ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "no provider selected")
                })
            };
        }

        if let Event::Key(key_event) = event::read()? {
            let item_rows = effective_viewport_rows(viewport_rows.max(1), picker.filtered_len());
            picker.handle_key(key_event, item_rows);
        }
    };

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}

#[cfg(test)]
#[path = "provider_picker_tests.rs"]
mod tests;
