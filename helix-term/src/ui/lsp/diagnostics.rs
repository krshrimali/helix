use std::sync::Arc;

use arc_swap::ArcSwap;
use helix_core::{diagnostic::Severity, syntax, Diagnostic};
use helix_view::graphics::{Margin, Rect, Style};
use helix_view::input::Event;
use tui::buffer::Buffer;
use tui::widgets::{BorderType, Paragraph, Widget, Wrap};

use crate::compositor::{Component, Context, EventResult};

use crate::alt;
use crate::ui::Markdown;

pub struct DiagnosticsPopup {
    active_index: usize,
    contents: Vec<DiagnosticContent>,
}

struct DiagnosticContent {
    header: Option<Markdown>,
    body: Markdown,
}

impl DiagnosticsPopup {
    pub const ID: &'static str = "diagnostics-popup";

    pub fn new(
        diagnostics: Vec<&Diagnostic>,
        config_loader: Arc<ArcSwap<syntax::Loader>>,
    ) -> Self {
        let n_diagnostics = diagnostics.len();
        let contents = diagnostics
            .into_iter()
            .enumerate()
            .map(|(idx, diag)| {
                let severity_str = match diag.severity() {
                    Severity::Error => "Error",
                    Severity::Warning => "Warning",
                    Severity::Info => "Info",
                    Severity::Hint => "Hint",
                };

                let source = diag.source.as_deref().unwrap_or("unknown");
                let code = diag
                    .code
                    .as_ref()
                    .map(|c| match c {
                        helix_core::diagnostic::NumberOrString::Number(n) => n.to_string(),
                        helix_core::diagnostic::NumberOrString::String(s) => s.clone(),
                    })
                    .unwrap_or_default();

                let header = if n_diagnostics > 1 {
                    Some(Markdown::new(
                        format!(
                            "**[{}/{}] {} ({}{}):**",
                            idx + 1,
                            n_diagnostics,
                            severity_str,
                            source,
                            if code.is_empty() {
                                String::new()
                            } else {
                                format!(" {}", code)
                            }
                        ),
                        config_loader.clone(),
                    ))
                } else {
                    Some(Markdown::new(
                        format!(
                            "**{} ({}{}):**",
                            severity_str,
                            source,
                            if code.is_empty() {
                                String::new()
                            } else {
                                format!(" {}", code)
                            }
                        ),
                        config_loader.clone(),
                    ))
                };

                let body = Markdown::new(diag.message.clone(), config_loader.clone());

                DiagnosticContent { header, body }
            })
            .collect();

        Self {
            active_index: 0,
            contents,
        }
    }

    fn has_header(&self) -> bool {
        self.contents
            .first()
            .map(|c| c.header.is_some())
            .unwrap_or(false)
    }

    fn content(&self) -> Option<&DiagnosticContent> {
        self.contents.get(self.active_index)
    }

    fn set_index(&mut self, index: usize) {
        if index < self.contents.len() {
            self.active_index = index;
        }
    }
}

const PADDING_HORIZONTAL: u16 = 2;
const PADDING_TOP: u16 = 1;
const PADDING_BOTTOM: u16 = 1;
const HEADER_HEIGHT: u16 = 1;
const SEPARATOR_HEIGHT: u16 = 1;

impl Component for DiagnosticsPopup {
    fn render(&mut self, area: Rect, surface: &mut Buffer, cx: &mut Context) {
        let margin = Margin::all(1);
        let area = area.inner(margin);

        let Some(content) = self.content() else {
            return;
        };

        // show header and border
        if let Some(ref header) = content.header {
            // header with severity
            let header = header.parse(Some(&cx.editor.theme));
            let header = Paragraph::new(&header);
            header.render(area.with_height(HEADER_HEIGHT), surface);

            // border
            let sep_style = Style::default();
            let borders = BorderType::line_symbols(BorderType::Plain);
            for x in area.left()..area.right() {
                if let Some(cell) = surface.get_mut(x, area.top() + HEADER_HEIGHT) {
                    cell.set_symbol(borders.horizontal).set_style(sep_style);
                }
            }
        }

        // diagnostic content
        let contents = content.body.parse(Some(&cx.editor.theme));
        let contents_area = area.clip_top(if self.has_header() {
            HEADER_HEIGHT + SEPARATOR_HEIGHT
        } else {
            0
        });
        let contents_para = Paragraph::new(&contents)
            .wrap(Wrap { trim: false })
            .scroll((cx.scroll.unwrap_or_default() as u16, 0));
        contents_para.render(contents_area, surface);
    }

    fn required_size(&mut self, viewport: (u16, u16)) -> Option<(u16, u16)> {
        let max_text_width = viewport.0.saturating_sub(PADDING_HORIZONTAL).clamp(10, 120);

        let Some(content) = self.content() else {
            return Some((20, 3));
        };

        let header_width = content
            .header
            .as_ref()
            .map(|header| {
                let header = header.parse(None);
                let (width, _height) = crate::ui::text::required_size(&header, max_text_width);
                width
            })
            .unwrap_or_default();

        let contents = content.body.parse(None);
        let (content_width, content_height) =
            crate::ui::text::required_size(&contents, max_text_width);

        let width = PADDING_HORIZONTAL + header_width.max(content_width);
        let height = if self.has_header() {
            PADDING_TOP + HEADER_HEIGHT + SEPARATOR_HEIGHT + content_height + PADDING_BOTTOM
        } else {
            PADDING_TOP + content_height + PADDING_BOTTOM
        };

        Some((width, height))
    }

    fn handle_event(&mut self, event: &Event, _ctx: &mut Context) -> EventResult {
        let Event::Key(event) = event else {
            return EventResult::Ignored(None);
        };

        match event {
            alt!('p') => {
                let index = self
                    .active_index
                    .checked_sub(1)
                    .unwrap_or(self.contents.len().saturating_sub(1));
                self.set_index(index);
                EventResult::Consumed(None)
            }
            alt!('n') => {
                self.set_index((self.active_index + 1) % self.contents.len().max(1));
                EventResult::Consumed(None)
            }
            _ => EventResult::Ignored(None),
        }
    }
}
