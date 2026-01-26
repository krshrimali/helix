use helix_core::syntax::config::LanguageServerFeature;
use helix_event::{cancelable_future, send_blocking, TaskController, TaskHandle};
use helix_lsp::lsp;
use helix_view::handlers::lsp::MouseHoverEvent;
use helix_view::{Editor, ViewId};
use tokio::time::Instant;

use crate::compositor::Compositor;
use crate::handlers::Handlers;
use crate::ui::lsp::hover::Hover;
use crate::ui::Popup;
use crate::job;

/// Pending hover request state
#[derive(Debug)]
struct PendingHover {
    view_id: ViewId,
    char_pos: usize,
}

#[derive(Debug)]
pub(super) struct MouseHoverHandler {
    pending: Option<PendingHover>,
    task_controller: TaskController,
}

impl MouseHoverHandler {
    pub fn new() -> MouseHoverHandler {
        MouseHoverHandler {
            pending: None,
            task_controller: TaskController::new(),
        }
    }
}

impl helix_event::AsyncHook for MouseHoverHandler {
    type Event = MouseHoverEvent;

    fn handle_event(
        &mut self,
        event: Self::Event,
        _timeout: Option<Instant>,
    ) -> Option<Instant> {
        match event {
            MouseHoverEvent::Moved {
                view_id,
                char_pos,
                delay,
            } => {
                // Check if position has changed
                if let Some(ref pending) = self.pending {
                    if pending.view_id == view_id && pending.char_pos == char_pos {
                        // Same position, don't restart timer
                        return None;
                    }
                }

                // Position changed - cancel any in-flight request to prevent
                // stale popups from appearing at wrong locations
                self.task_controller.cancel();

                // Don't close the existing popup here - let the user interact with it.
                // The popup will be replaced when the new hover request completes,
                // or closed via auto_close when user clicks outside/presses a key.

                // Store the new pending hover request
                self.pending = Some(PendingHover {
                    view_id,
                    char_pos,
                });

                // Return the debounce timeout using the configured delay
                Some(Instant::now() + delay)
            }
            MouseHoverEvent::Cancel => {
                self.pending = None;
                self.task_controller.cancel();
                // Close any existing hover popup
                job::dispatch_blocking(|_editor, compositor| {
                    compositor.remove(MOUSE_HOVER_ID);
                });
                None
            }
            MouseHoverEvent::RequestComplete { open: _ } => {
                self.task_controller.cancel();
                None
            }
        }
    }

    fn finish_debounce(&mut self) {
        let Some(pending) = self.pending.take() else {
            return;
        };

        let handle = self.task_controller.restart();
        job::dispatch_blocking(move |editor, _compositor| {
            request_mouse_hover(editor, pending, handle)
        });
    }
}

pub const MOUSE_HOVER_ID: &str = "mouse-hover";

fn request_mouse_hover(editor: &mut Editor, pending: PendingHover, cancel: TaskHandle) {
    // Check if mouse hover is enabled
    if !editor.config().lsp.mouse_hover {
        return;
    }

    let Some(view) = editor.tree.try_get(pending.view_id) else {
        return;
    };
    let doc_id = view.doc;
    let Some(doc) = editor.documents.get(&doc_id) else {
        return;
    };

    // Check if any language server supports hover
    if doc
        .language_servers_with_feature(LanguageServerFeature::Hover)
        .count()
        == 0
    {
        return;
    }

    let text = doc.text();
    let char_pos = pending.char_pos;

    // Ensure char_pos is valid
    if char_pos >= text.len_chars() {
        return;
    }

    let view_id = pending.view_id;

    // Build the futures for all language servers that support hover
    use futures_util::stream::FuturesOrdered;
    use std::collections::HashSet;

    let mut seen_language_servers = HashSet::new();
    let futures: FuturesOrdered<_> = doc
        .language_servers_with_feature(LanguageServerFeature::Hover)
        .filter(|ls| seen_language_servers.insert(ls.id()))
        .filter_map(|language_server| {
            let server_name = language_server.name().to_string();
            let offset_encoding = language_server.offset_encoding();

            // Convert position to LSP position
            let lsp_pos = helix_lsp::util::pos_to_lsp_pos(text, char_pos, offset_encoding);

            let request = language_server.text_document_hover(doc.identifier(), lsp_pos, None)?;

            Some(async move { anyhow::Ok((server_name, request.await?)) })
        })
        .collect();

    tokio::spawn(async move {
        use futures_util::StreamExt;

        if let Some(hovers) = cancelable_future(
            async {
                let mut hovers: Vec<(String, lsp::Hover)> = Vec::new();
                futures_util::pin_mut!(futures);

                while let Some(response) = futures.next().await {
                    match response {
                        Ok((server_name, Some(hover))) => hovers.push((server_name, hover)),
                        Ok(_) => (),
                        Err(err) => log::error!("Error requesting hover: {err}"),
                    }
                }
                hovers
            },
            cancel,
        )
        .await
        {
            job::dispatch(move |editor, compositor| {
                show_mouse_hover(editor, compositor, hovers, view_id, char_pos)
            })
            .await
        }
    });
}

fn show_mouse_hover(
    editor: &mut Editor,
    compositor: &mut Compositor,
    hovers: Vec<(String, lsp::Hover)>,
    view_id: ViewId,
    char_pos: usize,
) {
    send_blocking(
        &editor.handlers.mouse_hover,
        MouseHoverEvent::RequestComplete {
            open: !hovers.is_empty(),
        },
    );

    if hovers.is_empty() {
        // Only remove if the popup is not focused
        if let Some(popup) = compositor.find_id::<Popup<Hover>>(MOUSE_HOVER_ID) {
            if !popup.is_focused() {
                compositor.remove(MOUSE_HOVER_ID);
            }
        }
        return;
    }

    // Check if there's an existing focused popup - don't replace it
    if let Some(existing_popup) = compositor.find_id::<Popup<Hover>>(MOUSE_HOVER_ID) {
        if existing_popup.is_focused() {
            // User is interacting with the popup, don't replace it
            return;
        }
    }

    // Compute screen position from character position
    let position = editor
        .tree
        .try_get(view_id)
        .and_then(|view| {
            let doc = editor.documents.get(&view.doc)?;
            let text = doc.text().slice(..);
            view.screen_coords_at_pos(doc, text, char_pos)
        })
        .unwrap_or_default();

    // Create the hover popup with fixed positioning (won't follow cursor)
    let contents = Hover::new(hovers, editor.syn_loader.clone());
    let popup = Popup::new(MOUSE_HOVER_ID, contents)
        .position(Some(position))
        .fixed_position(true)
        .auto_close(true);

    compositor.replace_or_push(MOUSE_HOVER_ID, popup);
}

pub(super) fn register_hooks(_handlers: &Handlers) {
    // Mouse hover events are sent directly from the editor view,
    // so no hooks are needed here for now.
}
