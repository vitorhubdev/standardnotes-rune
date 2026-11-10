use std::time::Duration;

use rune::{El, ScrollSpec, TextStyle, col, el, row, text};
use rune_kit::{Kit, Markdown, TextArea, TextInput};

use crate::state::AppState;
use crate::views::{button, label};

pub fn view(s: &AppState, k: &Kit, height: f32) -> El {
    let Some(item) = s.selected() else {
        return col()
            .h_full()
            .grow(1.0)
            .center()
            .padding(32.0)
            .gap(12.0)
            .child(
                text("Deixe uma ideia por escrito")
                    .font(TextStyle::new(24.0, 500.0))
                    .color(k.c.text_base),
            )
            .child(
                label(
                    "Selecione uma nota ou comece uma nova.",
                    14.0,
                    k.c.text_muted,
                )
                .wrap(),
            );
    };
    let editable = s.can_edit();
    let is_note = item.content_type == "Note";
    let trashed = item.flag("trashed");
    let preview = s.open().is_some_and(|open| open.preview);
    let mut toolbar = row().w_full().gap(6.0).flex_wrap().center_y();
    if is_note {
        toolbar = toolbar
            .child(button(
                k,
                "toggle-pin",
                if item.flag("pinned") {
                    "Desafixar"
                } else {
                    "Fixar"
                },
                !s.busy(),
                |s, tx| s.toggle_flag("pinned", tx),
            ))
            .child(button(
                k,
                "toggle-archive",
                if item.flag("archived") {
                    "Desarquivar"
                } else {
                    "Arquivar"
                },
                !s.busy() && !trashed,
                |s, tx| s.toggle_flag("archived", tx),
            ))
            .child(button(
                k,
                "toggle-trash",
                if trashed {
                    "Restaurar"
                } else {
                    "Mover à lixeira"
                },
                !s.busy(),
                |s, tx| s.toggle_flag("trashed", tx),
            ));
    }
    if editable {
        toolbar = toolbar.child(button(
            k,
            "preview",
            if preview {
                "Editar texto"
            } else {
                "Prévia Markdown"
            },
            true,
            |s, _| {
                if let Some(open) = s.open_mut() {
                    open.preview = !open.preview;
                }
            },
        ));
    }
    let mut editor = col()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .padding(22.0)
        .gap(15.0)
        .child(toolbar);
    if editable {
        editor = editor
            .child(
                TextInput::new(&s.title)
                    .width(None)
                    .label("Título da nota")
                    .placeholder("Sem título")
                    .on_edit(|s: &mut AppState| &mut s.title)
                    .on_change(|s: &mut AppState, tx| s.edit_changed(tx))
                    .build(k)
                    .key("note-title"),
            )
            .child(
                row()
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .child(
                        TextInput::new(&s.tags)
                            .width(None)
                            .label("Etiquetas separadas por vírgula")
                            .placeholder("Etiquetas, separadas por vírgula")
                            .on_edit(|s: &mut AppState| &mut s.tags)
                            .on_submit(|s: &mut AppState, tx| s.apply_tags(tx))
                            .build(k)
                            .key("note-tags")
                            .grow(1.0),
                    )
                    .child(button(k, "apply-tags", "Aplicar", true, |s, tx| {
                        s.apply_tags(tx)
                    })),
            );
        let body = if preview {
            col()
                .key("markdown-preview")
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(Markdown::new(s.body.text.clone()).selectable(true).build(k))
        } else {
            TextArea::new(&s.body)
                .width(None)
                .label("Conteúdo da nota")
                .placeholder("Escreva aqui…")
                .rows(14)
                .max_height((height - 225.0).max(80.0))
                .on_edit(|s: &mut AppState| &mut s.body)
                .on_change(|s: &mut AppState, tx| s.edit_changed(tx))
                .build(k)
                .key("note-body")
                .w_full()
                .grow(1.0)
                .min_h(0.0)
        };
        editor = editor.child(body);
    } else {
        let name = if item.title().is_empty() {
            "Item preservado"
        } else {
            item.title()
        };
        let reason = if trashed {
            "Nota na lixeira. Restaure-a para voltar a editar."
        } else if item.flag("locked") || item.flag("protected") {
            "Esta nota está marcada como protegida no Standard Notes. Seu conteúdo permanece preservado."
        } else {
            "Este tipo de nota ou editor ainda não pode ser editado nesta versão. O conteúdo original é preservado na importação e na exportação."
        };
        editor = editor
            .child(
                text(name)
                    .font(TextStyle::new(24.0, 600.0))
                    .color(k.c.text_base)
                    .wrap(),
            )
            .child(label(reason, 14.0, k.c.text_muted).wrap())
            .child(label(
                format!(
                    "Formato: {} / {}",
                    item.content_type,
                    item.content
                        .get("noteType")
                        .and_then(|v| v.as_str())
                        .unwrap_or("original")
                ),
                12.0,
                k.c.text_muted,
            ));
        if !item.flag("locked") && !item.flag("protected") && !item.text().is_empty() {
            editor = editor.child(button(
                k,
                "copy-original",
                "Copiar conteúdo original",
                !s.busy(),
                |s, tx| {
                    if let Some(item) = s.selected() {
                        let _ = tx.copy_secret(item.text().to_owned(), Duration::from_secs(30));
                    }
                },
            ));
            let original: String = item.text().chars().take(4000).collect();
            editor = editor.child(
                col()
                    .grow(1.0)
                    .min_h(0.0)
                    .scroll(ScrollSpec {
                        stick_bottom: false,
                    })
                    .child(
                        label(original, 13.0, k.c.text_muted)
                            .wrap()
                            .selectable(true),
                    ),
            );
        }
    }
    let words = item.text().split_whitespace().count();
    editor.child(el().h(2.0)).child(
        label(
            format!(
                "{words} palavras · {} caracteres",
                item.text().chars().count()
            ),
            11.0,
            k.c.text_muted,
        )
        .key("note-stats"),
    )
}
