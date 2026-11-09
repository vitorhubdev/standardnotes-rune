use rune::{Cx, El, GrowthSpec, ScrollSpec, TextStyle, Tx, col, el, row, text};
use rune_kit::{Button, ButtonVariant, ControlSize, Kit, TextInput};

use crate::editor;
use crate::state::{AppState, Filter, IoPhase, SavePhase};

pub fn label(value: impl Into<String>, size: f32, color: rune::Color) -> El {
    text(value)
        .font(TextStyle::new(size, 400.0).lh(size * 1.45))
        .color(color)
}

pub fn button(
    k: &Kit,
    key: &str,
    title: &str,
    enabled: bool,
    action: impl Fn(&mut AppState, &mut Tx) + 'static,
) -> El {
    Button::new(title)
        .control_size(ControlSize::ExtraLarge)
        .disabled(!enabled)
        .on_press_tx(action)
        .build(k)
        .key(key)
}

pub fn view(s: &AppState, cx: &mut Cx<AppState>) -> El {
    let k = Kit::neutral(s.dark);
    cx.foreground(k.c.text_base);
    cx.focus_ring(2.0, 2.0, k.c.border_focus);
    let [width, height] = cx.layout_size("app").unwrap_or([1200.0, 800.0]);
    let body = if s.open().is_some() && s.backup_ui.mode.is_some() {
        crate::backup_ui::view(s, &k, height)
    } else if s.open().is_some() && s.account_ui.visible {
        crate::account::view(s, &k, height)
    } else if s.open().is_some() {
        workspace(s, &k, width, height)
    } else {
        locked(s, &k)
    };
    col()
        .key("app")
        .w_full()
        .h_full()
        .fill(k.c.page)
        .child(body)
}

fn locked(s: &AppState, k: &Kit) -> El {
    let waiting = s.busy();
    let title = if s.create {
        "Seu espaço começa aqui"
    } else {
        "Bem-vindo de volta"
    };
    let help = if s.create {
        "Crie uma senha para proteger suas notas neste dispositivo. Guarde-a: ela é necessária para abrir este cofre."
    } else {
        "Desbloqueie seu cofre para continuar de onde parou."
    };
    let mut form = col()
        .w_full()
        .gap(12.0)
        .child(label("STANDARD NOTES / RUNE", 12.0, k.c.text_muted))
        .child(
            text(title)
                .font(TextStyle::new(29.0, 600.0).lh(36.0))
                .color(k.c.text_base),
        )
        .child(label(help, 14.0, k.c.text_muted).wrap())
        .child(el().h(12.0))
        .child(label("Senha do cofre", 13.0, k.c.text_base))
        .child(
            TextInput::new(&s.password)
                .width(None)
                .label("Senha do cofre")
                .secure()
                .placeholder(if s.create {
                    "Pelo menos 8 caracteres"
                } else {
                    "Sua senha"
                })
                .disabled(waiting || s.demo)
                .on_edit(|s: &mut AppState| &mut s.password)
                .on_submit(|s: &mut AppState, _| s.unlock())
                .build(k)
                .key("password"),
        );
    if s.create {
        form = form
            .child(label("Confirme a senha", 13.0, k.c.text_base))
            .child(
                TextInput::new(&s.confirmation)
                    .width(None)
                    .label("Confirme a senha")
                    .secure()
                    .disabled(waiting)
                    .on_edit(|s: &mut AppState| &mut s.confirmation)
                    .on_submit(|s: &mut AppState, _| s.unlock())
                    .build(k)
                    .key("confirm-password"),
            );
    }
    form = form.child(
        Button::new(if waiting {
            "Aguarde…"
        } else if s.create {
            "Criar cofre"
        } else {
            "Desbloquear"
        })
        .variant(ButtonVariant::Contrast)
        .control_size(ControlSize::ExtraLarge)
        .disabled(waiting || s.demo)
        .on_press(|s: &mut AppState| s.unlock())
        .build(k)
        .key("unlock")
        .w_full(),
    );
    if let Some(error) = &s.error {
        form = form.child(error_banner(error, k));
    }
    form = form
        .child(el().h(4.0))
        .child(label(s.path.to_string_lossy(), 11.0, k.c.text_muted).truncate_middle())
        .child(
            row()
                .flex_wrap()
                .gap(8.0)
                .child(button(
                    k,
                    "choose-vault",
                    "Abrir outro cofre",
                    !waiting && !s.demo,
                    |s, tx| s.choose_vault(false, tx),
                ))
                .child(button(
                    k,
                    "new-vault",
                    "Novo cofre",
                    !waiting && !s.demo,
                    |s, tx| s.choose_vault(true, tx),
                )),
        )
        .child(
            label(
                "Aplicativo nativo · armazenamento local criptografado",
                12.0,
                k.c.text_muted,
            )
            .wrap(),
        );
    col().w_full().h_full().center().padding(24.0).child(
        col()
            .w_full()
            .max_w(480.0)
            .padding(28.0)
            .gap(12.0)
            .radius(16.0)
            .fill(k.c.layer_01)
            .border(1.0, k.c.border_base)
            .child(form),
    )
}

fn workspace(s: &AppState, k: &Kit, width: f32, height: f32) -> El {
    let compact = width < 820.0;
    let mut top = row()
        .w_full()
        .min_h(62.0)
        .px(18.0)
        .py(10.0)
        .gap(10.0)
        .center_y()
        .fill(k.c.layer_01)
        .border_sides([0.0, 0.0, 1.0, 0.0], k.c.border_base)
        .child(
            text("Standard Notes")
                .font(TextStyle::new(18.0, 600.0))
                .color(k.c.text_base),
        )
        .child(label("RUNE · PRÉVIA", 11.0, k.c.text_accent))
        .child(el().grow(1.0));
    if !compact && s.demo {
        top = top.child(label("Demonstração isolada", 12.0, k.c.text_muted));
    }
    if !s.demo && !compact {
        top = top
            .child(button(
                k,
                "import",
                "Importar",
                s.io == IoPhase::Idle && !s.busy(),
                |s, tx| s.import_dialog(tx),
            ))
            .child(button(
                k,
                "export",
                "Exportar",
                s.io == IoPhase::Idle && !s.busy(),
                |s, _| s.confirm_export = true,
            ));
    }
    if !s.demo {
        top = top.child(button(
            k,
            "account",
            "Conta",
            !s.busy() && s.io == IoPhase::Idle,
            crate::account::open,
        ));
        if !compact {
            top = top.child(button(
                k,
                "encrypted-export",
                "Backup com senha",
                !s.busy() && s.io == IoPhase::Idle,
                crate::backup_ui::open_export,
            ));
        }
    }
    top = top
        .child(button(
            k,
            "theme",
            if s.dark { "Claro" } else { "Escuro" },
            !s.busy(),
            |s, tx| s.toggle_theme(tx),
        ))
        .child(button(
            k,
            "lock",
            "Bloquear",
            !s.busy() && s.io == IoPhase::Idle,
            |s, tx| s.request_lock(tx),
        ));
    let middle = if compact {
        if s.compact_editor && s.selected().is_some() {
            col()
                .grow(1.0)
                .min_h(0.0)
                .child(button(
                    k,
                    "back-to-notes",
                    "Voltar às notas",
                    true,
                    |s, _| s.compact_editor = false,
                ))
                .child(editor::view(s, k, height - 155.0))
        } else {
            col()
                .grow(1.0)
                .min_h(0.0)
                .child(compact_filters(s, k))
                .child(notes(s, k).w_full().grow(1.0))
        }
    } else {
        row()
            .grow(1.0)
            .min_h(0.0)
            .child(sidebar(s, k).w(200.0))
            .child(notes(s, k).w(290.0))
            .child(editor::view(s, k, height - 120.0).grow(1.0).min_w(0.0))
    };
    let mut root = col().w_full().h_full().child(top).child(middle);
    if let Some(error) = &s.error {
        root = root.child(
            row()
                .w_full()
                .px(16.0)
                .py(8.0)
                .gap(8.0)
                .center_y()
                .child(error_banner(error, k).grow(1.0))
                .child(button(k, "dismiss-error", "Fechar aviso", true, |s, _| {
                    s.error = None
                })),
        );
    }
    if s.confirm_export {
        root = root.child(export_confirmation(k));
    }
    let retry = s
        .open()
        .is_some_and(|open| matches!(open.save, SavePhase::Failed));
    let mut footer = row()
        .w_full()
        .min_h(34.0)
        .px(16.0)
        .gap(8.0)
        .center_y()
        .fill(k.c.layer_01)
        .border_sides([1.0, 0.0, 0.0, 0.0], k.c.border_base)
        .child(
            label(
                &s.status,
                12.0,
                if retry { k.c.danger.fg } else { k.c.text_muted },
            )
            .truncate()
            .grow(1.0),
        );
    if retry {
        footer = footer.child(button(k, "retry-save", "Tentar salvar", true, |s, _| {
            s.save_now()
        }));
    }
    if !compact {
        footer = footer.child(label(
            "Ctrl+N  Nova nota     Ctrl+S  Salvar     Ctrl+L  Bloquear",
            11.0,
            k.c.text_muted,
        ));
    }
    root.child(footer)
}

fn error_banner(message: &str, k: &Kit) -> El {
    label(message, 13.0, k.c.danger.fg)
        .wrap()
        .role("alert")
        .key("error")
}

fn export_confirmation(k: &Kit) -> El {
    col().key("export-confirmation").w_full().padding(18.0).gap(10.0).fill(k.c.layer_02)
        .child(text("Exportar sem criptografia?").font(TextStyle::new(16.0, 600.0)).color(k.c.text_base))
        .child(label("O arquivo JSON conterá suas notas legíveis. Qualquer pessoa com acesso ao arquivo poderá lê-las. Guarde-o em um local protegido.", 13.0, k.c.text_muted).wrap())
        .child(row().gap(10.0).flex_wrap()
            .child(button(k, "confirm-export", "Escolher destino e exportar", true, |s, tx| s.export_dialog(tx)))
            .child(button(k, "cancel-export", "Cancelar", true, |s, _| s.confirm_export = false)))
}

fn filter_button(s: &AppState, k: &Kit, filter: Filter, key: &str) -> El {
    let selected = s.open().is_some_and(|open| open.filter == filter);
    let name = filter.label().to_owned();
    Button::new(name)
        .variant(if selected {
            ButtonVariant::Contrast
        } else {
            ButtonVariant::Ghost
        })
        .control_size(ControlSize::ExtraLarge)
        .disabled(s.busy())
        .on_press(move |s: &mut AppState| s.filter(filter.clone()))
        .build(k)
        .key(key)
}

fn compact_filters(s: &AppState, k: &Kit) -> El {
    row()
        .w_full()
        .padding(10.0)
        .gap(6.0)
        .flex_wrap()
        .child(filter_button(s, k, Filter::Notes, "filter-all"))
        .child(filter_button(s, k, Filter::Pinned, "filter-pinned"))
        .child(filter_button(s, k, Filter::Archived, "filter-archived"))
        .child(filter_button(s, k, Filter::Trash, "filter-trash"))
        .child(filter_button(s, k, Filter::Other, "filter-other"))
}

fn sidebar(s: &AppState, k: &Kit) -> El {
    let mut content = col()
        .w_full()
        .padding(12.0)
        .gap(6.0)
        .child(label("SEU CADERNO", 11.0, k.c.text_muted).py(8.0))
        .child(filter_button(s, k, Filter::Notes, "filter-all").w_full())
        .child(filter_button(s, k, Filter::Pinned, "filter-pinned").w_full())
        .child(filter_button(s, k, Filter::Archived, "filter-archived").w_full())
        .child(filter_button(s, k, Filter::Trash, "filter-trash").w_full())
        .child(filter_button(s, k, Filter::Other, "filter-other").w_full())
        .child(el().h(16.0))
        .child(label("ETIQUETAS", 11.0, k.c.text_muted).py(8.0));
    if let Some(open) = s.open() {
        let mut tags: Vec<_> = open
            .data
            .items
            .iter()
            .filter(|item| item.is_tag() && !item.deleted)
            .collect();
        tags.sort_by_key(|item| item.title().to_lowercase());
        tags.dedup_by(|a, b| a.title() == b.title());
        for tag in tags {
            content = content.child(
                filter_button(
                    s,
                    k,
                    Filter::Tag(tag.title().into()),
                    &format!("tag-{}", tag.uuid),
                )
                .w_full(),
            );
        }
    }
    col()
        .h_full()
        .min_h(0.0)
        .fill(k.c.layer_01)
        .border_sides([0.0, 1.0, 0.0, 0.0], k.c.border_base)
        .child(
            col()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(content),
        )
}

fn notes(s: &AppState, k: &Kit) -> El {
    let count = s.open().map_or(0, |open| open.visible.len());
    let heading = s.open().map_or("Notas", |open| open.filter.label());
    let header = col()
        .w_full()
        .padding(14.0)
        .gap(12.0)
        .child(
            row()
                .center_y()
                .gap(8.0)
                .child(
                    text(heading)
                        .font(TextStyle::new(16.0, 600.0))
                        .color(k.c.text_base)
                        .truncate()
                        .grow(1.0),
                )
                .child(label(count.to_string(), 12.0, k.c.text_muted)),
        )
        .child(
            Button::new("Nova nota")
                .variant(ButtonVariant::Contrast)
                .control_size(ControlSize::ExtraLarge)
                .disabled(s.busy())
                .on_press_tx(|s: &mut AppState, tx: &mut Tx| s.new_note(tx))
                .build(k)
                .key("new-note")
                .w_full(),
        )
        .child(
            TextInput::new(&s.search)
                .width(None)
                .placeholder("Buscar nas notas")
                .label("Buscar nas notas")
                .on_edit(|s: &mut AppState| &mut s.search)
                .on_change(|s: &mut AppState, _| s.refresh())
                .build(k)
                .key("search"),
        );
    let rows = col()
        .key("note-rows")
        .growth(GrowthSpec::default())
        .virtual_rows(
            count,
            96.0,
            |s: &AppState, i| {
                s.open()
                    .and_then(|open| {
                        open.visible
                            .get(i)
                            .map(|&idx| open.data.items[idx].uuid.as_str().into())
                    })
                    .unwrap_or_else(|| "missing".into())
            },
            |s: &AppState, i| note_row(s, i),
        );
    let list = if count == 0 {
        col()
            .grow(1.0)
            .padding(24.0)
            .gap(10.0)
            .child(label("Nenhuma nota aqui", 16.0, k.c.text_base))
            .child(
                label(
                    "Crie uma nota, importe um backup ou ajuste a busca.",
                    13.0,
                    k.c.text_muted,
                )
                .wrap(),
            )
    } else {
        col()
            .key("note-scroll")
            .role("listbox")
            .label("Notas")
            .focus_group()
            .grow(1.0)
            .min_h(0.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .child(rows)
    };
    col()
        .h_full()
        .min_h(0.0)
        .fill(k.c.page)
        .border_sides([0.0, 1.0, 0.0, 0.0], k.c.border_base)
        .child(header)
        .child(list)
}

fn note_row(s: &AppState, i: usize) -> El {
    let k = Kit::neutral(s.dark);
    let Some(open) = s.open() else {
        return el();
    };
    let Some(&index) = open.visible.get(i) else {
        return el();
    };
    let item = &open.data.items[index];
    let selected = open.selected.as_deref() == Some(item.uuid.as_str());
    let id = item.uuid.clone();
    let title = if item.title().is_empty() {
        "Sem título"
    } else {
        item.title()
    };
    let preview: String = if item.flag("locked") || item.flag("protected") {
        "Nota protegida".into()
    } else {
        item.text()
            .lines()
            .find(|x| !x.trim().is_empty())
            .unwrap_or("Nota vazia")
            .chars()
            .take(100)
            .collect()
    };
    let detail = if item.flag("pinned") {
        "Fixada".to_owned()
    } else {
        item.modified_at().chars().take(10).collect()
    };
    col()
        .h(96.0)
        .w_full()
        .padding(14.0)
        .gap(5.0)
        .role("option")
        .focus_selected(selected)
        .label(title)
        .fill(if selected { k.c.layer_03 } else { k.c.page })
        .hover(k.c.hover)
        .focusable(true)
        .focus_ring(2.0, -2.0, k.c.border_focus)
        .border_sides(
            [0.0, 0.0, 1.0, if selected { 3.0 } else { 0.0 }],
            if selected {
                k.c.text_accent
            } else {
                k.c.border_base
            },
        )
        .child(
            text(title)
                .font(TextStyle::new(14.0, 600.0))
                .color(k.c.text_base)
                .truncate(),
        )
        .child(label(preview, 12.0, k.c.text_muted).truncate())
        .child(label(detail, 10.0, k.c.text_muted).truncate())
        .on_press_tx(move |s: &mut AppState, tx: &mut Tx| {
            s.apply_tags(tx);
            s.select(&id);
        })
}
