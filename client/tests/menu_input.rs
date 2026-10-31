//! Authored synthetic editor/event traces; no OS input, clipboard or window.
use slither_client::{
    desktop_input::{DesktopInput, OwnInput},
    menu_input::*,
    menu_preferences::MenuQuality,
    start_page::{MenuAction, MenuLocale, StartMenuIntent, StartMenuState},
};

fn editor(field: TextField, text: &str) -> MenuInputState {
    let mut input = MenuInputState::default();
    input.apply(EditIntent::Focus(Some(field)));
    input.apply(EditIntent::KeyText(text.into()));
    input
}
fn text(input: &MenuInputState, field: TextField) -> &str {
    input.editor(field).text()
}
fn enter(pressed: bool, repeat: bool) -> EditIntent {
    EditIntent::Enter { pressed, repeat }
}
fn paste(input: &mut MenuInputState, value: Result<String, String>) -> Vec<HostEditEffect> {
    let effects = input.apply(EditIntent::Paste);
    let HostEditEffect::ReadClipboard(token) = effects[0] else {
        panic!("expected read")
    };
    input.apply(EditIntent::ClipboardRead {
        token,
        result: value,
    })
}

#[test]
fn utf8_utf16_offsets_reject_interior_bytes_and_surrogates() {
    let s = "a😀e\u{301}界";
    for (byte, unit) in [(0, 0), (1, 1), (5, 3), (6, 4), (8, 5), (11, 6)] {
        assert_eq!(byte_to_utf16(s, byte), Some(unit));
        assert_eq!(utf16_to_byte(s, unit), Some(byte));
    }
    for byte in [2, 3, 4, 7, 9, 10, 12, usize::MAX] {
        assert_eq!(byte_to_utf16(s, byte), None);
    }
    for unit in [2, 7, usize::MAX] {
        assert_eq!(utf16_to_byte(s, unit), None);
    }
    assert_eq!(utf16_to_byte("", 0), Some(0));
}

#[test]
fn normalization_is_identical_for_typing_paste_and_ime_commit() {
    for field in [TextField::Nickname, TextField::Champion] {
        let raw = "x\0\u{7f}é😀e\u{301}界".repeat(30);
        let typed = editor(field, &raw);
        let mut pasted = editor(field, "");
        assert!(paste(&mut pasted, Ok(raw.clone())).is_empty());
        let mut ime = editor(field, "");
        ime.apply(EditIntent::Preedit(raw.clone(), None));
        assert_eq!(text(&ime, field), "");
        assert_eq!(ime.editor(field).preedit().unwrap().text, raw);
        ime.apply(EditIntent::ImeCommit(raw));
        assert_eq!(text(&typed, field), text(&pasted, field));
        assert_eq!(text(&typed, field), text(&ime, field));
        let limit = if field == TextField::Nickname {
            24
        } else {
            140
        };
        assert!(text(&typed, field).encode_utf16().count() <= limit);
    }
    assert_eq!(
        text(
            &editor(TextField::Nickname, "a\né\u{7f}😀"),
            TextField::Nickname
        ),
        "a  \u{7f}  "
    );
    assert_eq!(
        text(
            &editor(TextField::Nickname, &("n".repeat(23) + "😀")),
            TextField::Nickname
        ),
        "n".repeat(23) + " "
    );
    assert_eq!(
        text(
            &editor(TextField::Champion, &("c".repeat(139) + "😀")),
            TextField::Champion
        ),
        "c".repeat(139) + " "
    );
    assert_eq!(
        text(
            &editor(TextField::Champion, &("c".repeat(138) + "😀x")),
            TextField::Champion
        ),
        "c".repeat(138) + "  "
    );
    assert_eq!(
        text(
            &editor(TextField::Champion, "é😀e\u{301}"),
            TextField::Champion
        ),
        "   e "
    );
}

#[test]
fn champion_ascii_units_and_140_limit_apply_to_typing_paste_and_ime() {
    let field = TextField::Champion;
    for (raw, expected) in [
        (
            " Aé😀e\u{301}\u{1f}\u{7f}!".into(),
            " A   e  \u{7f}!".into(),
        ),
        ("😀".repeat(71), " ".repeat(140)),
        ("c".repeat(139) + "😀", "c".repeat(139) + " "),
        ("c".repeat(138) + "😀x", "c".repeat(138) + "  "),
    ] {
        assert_eq!(text(&editor(field, &raw), field), expected);
        let mut pasted = editor(field, "");
        paste(&mut pasted, Ok(raw.clone()));
        assert_eq!(text(&pasted, field), expected);
        let mut ime = editor(field, "");
        ime.apply(EditIntent::Preedit(raw.clone(), None));
        assert_eq!(ime.editor(field).preedit().unwrap().text, raw);
        assert_eq!(text(&ime, field), "");
        ime.apply(EditIntent::ImeCommit(raw));
        assert_eq!(text(&ime, field), expected);
        assert!(ime.editor(field).preedit().is_none());
    }
}

#[test]
fn selection_replacement_normalizes_the_whole_result_then_limits() {
    let mut input = editor(TextField::Nickname, "abcdefghijklmnopqrstuvwx");
    input.apply(EditIntent::Select {
        anchor: 1,
        caret: 3,
    });
    paste(&mut input, Ok("😀z".into()));
    assert_eq!(
        text(&input, TextField::Nickname),
        "a  zdefghijklmnopqrstuvw"
    );
    assert_eq!(input.editor(TextField::Nickname).selection(), (4, 4));
}

#[test]
fn raw_preedit_commit_and_composition_enter_are_separate() {
    let field = TextField::Champion;
    let mut input = editor(field, "old");
    input.apply(EditIntent::SelectAll);
    input.apply(EditIntent::ImeEnabled(true));
    input.apply(EditIntent::Preedit("界😀".into(), Some((3, 7))));
    assert_eq!(
        input.editor(field).preedit().unwrap().selection,
        Some((3, 7))
    );
    // Defensive out-of-contract keyboard text must not touch the raw preview.
    input.apply(EditIntent::KeyText("unexpected".into()));
    assert_eq!(text(&input, field), "old");
    assert_eq!(input.editor(field).preedit().unwrap().text, "界😀");
    assert!(input.apply(EditIntent::ImeEnterPressed).is_empty());
    input.apply(EditIntent::Preedit("".into(), None));
    assert!(!input.editor(field).composing());
    input.apply(EditIntent::ImeCommit("界😀".into()));
    input.apply(EditIntent::ImeEnabled(false));
    assert_eq!(text(&input, field), "   ");
    assert!(input.apply(enter(true, true)).is_empty());
    input.apply(enter(false, false));
    assert_eq!(
        input.apply(enter(true, false)),
        vec![HostEditEffect::Activate(field)]
    );
    assert!(input.apply(enter(true, true)).is_empty());
    input.apply(EditIntent::Undo);
    assert_eq!(text(&input, field), "old");
}

#[test]
fn ime_commit_always_inserts_and_cancel_discards_only_preedit() {
    let field = TextField::Nickname;
    let mut input = editor(field, "a");
    input.apply(EditIntent::Preedit("é".into(), Some((1, usize::MAX))));
    assert_eq!(
        input.editor(field).preedit().unwrap().selection,
        Some((0, 2))
    );
    input.apply(EditIntent::ImeCommit("é".into()));
    input.apply(EditIntent::ImeEnabled(false));
    assert_eq!(
        input.apply(enter(true, false)),
        vec![HostEditEffect::Activate(field)]
    );
    assert_eq!(text(&input, field), "a ");
    input.apply(enter(false, false));
    input.apply(EditIntent::Preedit("discard".into(), None));
    input.apply(EditIntent::CancelComposition);
    assert!(!input.editor(field).composing());
    assert_eq!(text(&input, field), "a ");
    input.apply(EditIntent::ImeCommit("b".into()));
    assert_eq!(text(&input, field), "a b");
    input.apply(EditIntent::Preedit("also discard".into(), None));
    input.apply(EditIntent::ImeEnabled(false));
    assert!(input.editor(field).preedit().is_none());
    input.apply(EditIntent::ImeEnabled(true));
    input.apply(EditIntent::KeyText("z".into()));
    assert_eq!(text(&input, field), "a bz");
    input.apply(EditIntent::Preedit("cancel without Disabled".into(), None));
    input.apply(EditIntent::Preedit("".into(), None));
    assert!(!input.editor(field).composing());
    input.apply(EditIntent::KeyText("x".into()));
    assert_eq!(text(&input, field), "a bzx");
}

#[test]
fn round3_key_text_and_ime_commits_are_always_distinct_edits() {
    for field in [TextField::Nickname, TextField::Champion] {
        for text_first in [true, false] {
            let mut input = editor(field, "");
            if text_first {
                input.apply(EditIntent::KeyText("a".into()));
            }
            input.apply(EditIntent::ImeCommit("a".into()));
            if !text_first {
                input.apply(EditIntent::KeyText("a".into()));
            }
            assert_eq!(text(&input, field), "aa");
            input.apply(EditIntent::Undo);
            assert_eq!(text(&input, field), "a");
            input.apply(EditIntent::Undo);
            assert_eq!(text(&input, field), "");
        }
    }
}

#[test]
fn round3_composition_enter_release_then_fresh_a_keeps_both_edits() {
    for field in [TextField::Nickname, TextField::Champion] {
        let mut input = editor(field, "");
        input.apply(EditIntent::ImeEnabled(true));
        input.apply(EditIntent::Preedit("a".into(), None));
        assert!(input.apply(EditIntent::ImeEnterPressed).is_empty());
        input.apply(EditIntent::Preedit("".into(), None));
        input.apply(EditIntent::ImeCommit("a".into()));
        input.apply(EditIntent::ImeEnabled(false));
        input.apply(enter(false, false));
        input.apply(EditIntent::KeyText("a".into()));
        assert_eq!(text(&input, field), "aa");
        assert_eq!(
            input.apply(enter(true, false)),
            vec![HostEditEffect::Activate(field)]
        );
    }
}

#[test]
fn round3_ordinary_a_then_direct_commit_with_empty_preedit_keeps_both() {
    let field = TextField::Nickname;
    let mut input = editor(field, "a");
    input.apply(EditIntent::ImeEnabled(true));
    input.apply(EditIntent::Preedit("".into(), None));
    input.apply(EditIntent::ImeCommit("a".into()));
    assert_eq!(text(&input, field), "aa");
}

#[test]
fn ordinary_edit_then_new_composition_keeps_both() {
    let field = TextField::Champion;
    let mut input = editor(field, "a");
    input.apply(EditIntent::ImeEnabled(true));
    input.apply(EditIntent::Preedit("a".into(), None));
    input.apply(EditIntent::ImeEnterPressed);
    input.apply(EditIntent::Preedit("".into(), None));
    input.apply(EditIntent::ImeCommit("a".into()));
    assert_eq!(text(&input, field), "aa");
    input.apply(EditIntent::Undo);
    assert_eq!(text(&input, field), "a");
}

#[test]
fn round3_committed_a_and_later_keys_preserve_aa_and_aba() {
    for (keys, expected) in [
        (vec!["a"], "aa"),
        (vec!["b", "a"], "aba"),
        (vec!["a", "b", "a"], "aaba"),
    ] {
        let mut input = editor(TextField::Nickname, "");
        input.apply(EditIntent::ImeEnabled(true));
        input.apply(EditIntent::ImeCommit("a".into()));
        for key in keys {
            input.apply(EditIntent::KeyText(key.into()));
        }
        assert_eq!(text(&input, TextField::Nickname), expected);
    }
}

#[test]
fn round3_interleaved_compositions_preserve_each_commit_and_history() {
    let field = TextField::Champion;
    let mut input = editor(field, "");
    for _ in 0..2 {
        input.apply(EditIntent::Preedit("a".into(), None));
        input.apply(EditIntent::Preedit("".into(), None));
        input.apply(EditIntent::ImeCommit("a".into()));
    }
    input.apply(EditIntent::KeyText("b".into()));
    input.apply(EditIntent::KeyText("a".into()));
    assert_eq!(text(&input, field), "aaba");
    for expected in ["aab", "aa", "a", ""] {
        input.apply(EditIntent::Undo);
        assert_eq!(text(&input, field), expected);
    }
}

#[test]
fn composition_enter_barrier_survives_preedit_clear_but_clears_on_blur() {
    let mut input = editor(TextField::Nickname, "");
    input.apply(EditIntent::Preedit("a".into(), None));
    input.apply(EditIntent::ImeEnterPressed);
    input.apply(EditIntent::Preedit("".into(), None));
    input.apply(EditIntent::ImeCommit("a".into()));
    assert!(input.apply(enter(true, false)).is_empty());
    input.apply(EditIntent::WindowFocused(false));
    input.apply(EditIntent::WindowFocused(true));
    assert_eq!(
        input.apply(enter(true, false)),
        vec![HostEditEffect::Activate(TextField::Nickname)]
    );
}

#[test]
fn round4_no_consumed_enter_means_the_first_enter_activates() {
    for field in [TextField::Nickname, TextField::Champion] {
        for steps in [
            vec![
                EditIntent::Preedit("a".into(), None),
                EditIntent::Preedit("".into(), None),
                EditIntent::ImeCommit("a".into()),
                EditIntent::ImeEnabled(false),
            ],
            vec![EditIntent::ImeCommit("a".into())],
            vec![
                EditIntent::Preedit("cancel".into(), None),
                EditIntent::Preedit("".into(), None),
            ],
            vec![
                EditIntent::Preedit("cancel".into(), None),
                EditIntent::ImeEnabled(false),
            ],
        ] {
            let mut input = editor(field, "ready");
            input.apply(EditIntent::ImeEnabled(true));
            for step in steps {
                input.apply(step);
            }
            assert_eq!(
                input.apply(enter(true, false)),
                vec![HostEditEffect::Activate(field)]
            );
        }
    }
}

#[test]
fn logical_enter_repeats_never_activate_play_or_save() {
    for field in [TextField::Nickname, TextField::Champion] {
        let mut input = editor(field, "ready");
        assert!(input.apply(enter(true, true)).is_empty());
        assert_eq!(
            input.apply(enter(true, false)),
            vec![HostEditEffect::Activate(field)]
        );
        assert!(input.apply(enter(true, true)).is_empty());
        input.apply(enter(false, false));
        assert_eq!(
            input.apply(enter(true, false)),
            vec![HostEditEffect::Activate(field)]
        );
    }
}

#[test]
fn round2_nickname_last_unit_documentation_matches_surrogate_limit() {
    for (prefix_units, spaces) in [(22, 2), (23, 1)] {
        let value = editor(TextField::Nickname, &("n".repeat(prefix_units) + "😀"));
        assert_eq!(
            text(&value, TextField::Nickname),
            "n".repeat(prefix_units) + &" ".repeat(spaces)
        );
    }
    let docs = include_str!("../docs/start-menu.md")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(docs.contains(
        "an emoji becomes two spaces, or one space at unit 24 when the limit splits the pair."
    ));
}

#[test]
fn incremental_typing_stops_at_utf16_limit_and_selected_text_remains_replaceable() {
    for (field, limit) in [(TextField::Nickname, 24), (TextField::Champion, 140)] {
        let mut input = editor(field, "");
        for _ in 0..limit + 2 {
            input.apply(EditIntent::KeyText("n".into()));
        }
        assert_eq!(text(&input, field), "n".repeat(limit));
        input.apply(EditIntent::KeyText("😀".into()));
        assert_eq!(text(&input, field), "n".repeat(limit));
        input.apply(EditIntent::Select {
            anchor: 0,
            caret: 2,
        });
        input.apply(EditIntent::KeyText("😀".into()));
        assert_eq!(
            text(&input, field),
            "  ".to_owned() + &"n".repeat(limit - 2)
        );
        assert_eq!(text(&input, field).encode_utf16().count(), limit);
    }
}

#[test]
fn selections_arrows_shift_home_end_deletion_and_history() {
    let field = TextField::Champion;
    let mut input = editor(field, "aBCde z");
    input.apply(EditIntent::Move {
        movement: Movement::Home,
        extend: false,
    });
    input.apply(EditIntent::Move {
        movement: Movement::Right,
        extend: true,
    });
    input.apply(EditIntent::Move {
        movement: Movement::Right,
        extend: true,
    });
    assert_eq!(input.editor(field).selection(), (0, 2));
    input.apply(EditIntent::Move {
        movement: Movement::Left,
        extend: false,
    });
    assert_eq!(input.editor(field).selection(), (0, 0));
    input.apply(EditIntent::Move {
        movement: Movement::End,
        extend: true,
    });
    input.apply(EditIntent::KeyText("new".into()));
    assert_eq!(text(&input, field), "new");
    input.apply(EditIntent::Undo);
    assert_eq!(input.editor(field).selection(), (0, 7));
    input.apply(EditIntent::Redo);
    assert_eq!(text(&input, field), "new");
    input.apply(EditIntent::Backspace);
    assert_eq!(text(&input, field), "ne");
    input.apply(EditIntent::Undo);
    assert_eq!(input.editor(field).selection(), (3, 3));
    input.apply(EditIntent::Move {
        movement: Movement::Home,
        extend: false,
    });
    input.apply(EditIntent::Delete);
    assert_eq!(text(&input, field), "ew");
    input.apply(EditIntent::Undo);
    input.apply(EditIntent::KeyText("x".into()));
    input.apply(EditIntent::Redo);
    assert_eq!(text(&input, field), "xnew");
}

#[test]
fn normalized_combining_marks_and_words_use_committed_byte_boundaries() {
    let field = TextField::Champion;
    let mut input = editor(field, "hi  e\u{301} 😀");
    assert_eq!(text(&input, field), "hi  e    ");
    input.apply(EditIntent::Move {
        movement: Movement::WordLeft,
        extend: false,
    });
    assert_eq!(input.editor(field).selection(), (4, 4));
    input.apply(EditIntent::Move {
        movement: Movement::WordLeft,
        extend: false,
    });
    assert_eq!(input.editor(field).selection(), (0, 0));
    input.apply(EditIntent::Move {
        movement: Movement::WordRight,
        extend: false,
    });
    assert_eq!(input.editor(field).selection(), (4, 4));
    input.apply(EditIntent::Select {
        anchor: 6,
        caret: 6,
    });
    input.apply(EditIntent::Backspace);
    assert_eq!(text(&input, field), "hi  e   ");
    input.apply(EditIntent::Select {
        anchor: 999,
        caret: 6,
    });
    assert_eq!(input.editor(field).selection(), (8, 6));
}

#[test]
fn clipboard_failure_is_reported_cut_is_transactional_and_stale_results_are_ignored() {
    let field = TextField::Nickname;
    let mut input = editor(field, "copy me");
    input.apply(EditIntent::SelectAll);
    let effects = input.apply(EditIntent::Cut);
    let HostEditEffect::WriteClipboard { token, ref text } = effects[0] else {
        panic!()
    };
    assert_eq!(text, "copy me");
    assert_eq!(
        input.apply(EditIntent::ClipboardWritten {
            token,
            result: Err("busy".into())
        }),
        vec![HostEditEffect::ClipboardFailed("busy".into())]
    );
    assert_eq!(input.editor(field).text(), "copy me");
    let effects = input.apply(EditIntent::Cut);
    let HostEditEffect::WriteClipboard { token, .. } = effects[0] else {
        panic!()
    };
    input.apply(EditIntent::ClipboardWritten {
        token,
        result: Ok(()),
    });
    assert_eq!(input.editor(field).text(), "");
    input.apply(EditIntent::Undo);
    assert_eq!(input.editor(field).text(), "copy me");
    let effects = input.apply(EditIntent::Paste);
    let HostEditEffect::ReadClipboard(token) = effects[0] else {
        panic!()
    };
    input.apply(EditIntent::Focus(Some(TextField::Champion)));
    input.apply(EditIntent::ClipboardRead {
        token,
        result: Ok("wrong field".into()),
    });
    assert_eq!(input.editor(TextField::Champion).text(), "");
    assert_eq!(
        paste(&mut input, Err("unavailable".into())),
        vec![HostEditEffect::ClipboardFailed("unavailable".into())]
    );
    input.apply(EditIntent::Focus(Some(field)));
    let effects = input.apply(EditIntent::Cut);
    let HostEditEffect::WriteClipboard { token, .. } = effects[0] else {
        panic!()
    };
    input.apply(EditIntent::Move {
        movement: Movement::Home,
        extend: false,
    });
    input.apply(EditIntent::ClipboardWritten {
        token,
        result: Ok(()),
    });
    assert_eq!(input.editor(field).text(), "copy me");
}

fn gameplay() -> DesktopInput {
    let mut game = DesktopInput::new(19.).unwrap();
    game.set_own(
        OwnInput {
            wanted_heading: 0.,
            gaze_heading: 0.,
            dead: false,
            turn_rate: 0.03,
            size_turn_factor: 1.,
            speed_turn_factor: 1.,
            requested_boost: false,
            sent_boost: false,
        },
        false,
    )
    .unwrap();
    game
}
fn deliver(game: &mut DesktopInput, effects: Vec<HostEditEffect>) {
    for effect in effects {
        match effect {
            HostEditEffect::Gameplay { key, pressed } => {
                game.key(key.name(), pressed);
            }
            HostEditEffect::ReleasePointerBoost => game.mouse_up(),
            _ => {}
        }
    }
}
#[test]
fn focus_modal_and_blur_clear_held_turn_and_boost_without_changing_legacy_policy() {
    for boundary in [
        EditIntent::Focus(Some(TextField::Nickname)),
        EditIntent::Modal(true),
        EditIntent::WindowFocused(false),
    ] {
        let mut game = gameplay();
        let mut input = MenuInputState::default();
        for key in [
            GameplayKey::Left,
            GameplayKey::Right,
            GameplayKey::Up,
            GameplayKey::Space,
        ] {
            deliver(
                &mut game,
                input.apply(EditIntent::Gameplay { key, pressed: true }),
            );
        }
        assert!(game.view().left_held && game.view().right_held && game.view().boost_key_held);
        deliver(&mut game, input.apply(boundary));
        assert!(!game.view().left_held && !game.view().right_held && !game.view().boost_key_held);
        assert!(!game.view().own.unwrap().requested_boost);
        assert!(
            input
                .apply(EditIntent::Gameplay {
                    key: GameplayKey::Left,
                    pressed: true
                })
                .is_empty()
        );
        assert_eq!(
            input.apply(EditIntent::Gameplay {
                key: GameplayKey::Left,
                pressed: false
            }),
            vec![HostEditEffect::Gameplay {
                key: GameplayKey::Left,
                pressed: false
            }]
        );
    }
    let mut input = editor(TextField::Nickname, "name");
    input.apply(EditIntent::Preedit("discard".into(), None));
    input.apply(EditIntent::Modal(true));
    assert_eq!(input.focus(), None);
    input.apply(EditIntent::Modal(false));
    assert_eq!(input.focus(), Some(TextField::Nickname));
    assert!(!input.editor(TextField::Nickname).composing());
}

#[test]
fn menu_focus_and_enter_keep_deferred_play_and_admission_authority() {
    let mut menu = StartMenuState::new(MenuLocale::English, false, MenuQuality::High);
    let mut input = MenuInputState::default();
    let intents = menu.step(0., false).unwrap();
    input.sync_menu(&menu.view(true), &intents);
    assert_eq!(input.focus(), Some(TextField::Nickname));
    input.apply_menu(EditIntent::KeyText("é😀 name".into()), &mut menu, true);
    input.apply_menu(EditIntent::Preedit("候補".into(), None), &mut menu, true);
    input.apply_menu(EditIntent::ImeEnterPressed, &mut menu, true);
    assert!(menu.step(8., false).unwrap().is_empty());
    input.apply_menu(EditIntent::ImeCommit("x".into()), &mut menu, true);
    input.apply_menu(enter(false, false), &mut menu, true);
    input.apply_menu(enter(true, false), &mut menu, true);
    assert_eq!(input.focus(), None);
    assert_eq!(
        menu.step(16., false).unwrap(),
        vec![StartMenuIntent::Play {
            nickname: "    namex".into(),
            quality: MenuQuality::High
        }]
    );
    assert!(menu.step(24., false).unwrap().is_empty());
    let mut blocked = StartMenuState::new(MenuLocale::English, false, MenuQuality::High);
    input.apply_menu(enter(true, false), &mut blocked, false);
    assert!(
        blocked
            .step(0., false)
            .unwrap()
            .iter()
            .all(|i| !matches!(i, StartMenuIntent::Play { .. }))
    );
    blocked.action(MenuAction::Play, false);
    assert!(!blocked.view(false).spinner_visible);
}

#[test]
fn login_semantic_tree_snapshots_include_values_focus_actions_and_busy_state() {
    let mut menu = StartMenuState::new(MenuLocale::French, false, MenuQuality::High);
    menu.edit_nickname(&"Ada".encode_utf16().collect::<Vec<_>>());
    let mut input = MenuInputState::default();
    input.sync_menu(&menu.view(true), &[StartMenuIntent::FocusNickname]);
    let nodes = login_accessibility(&menu.view(true), &input);
    assert_eq!(
        nodes
            .iter()
            .map(|n| (
                n.id,
                n.role,
                n.name.as_str(),
                n.value.as_str(),
                n.focused,
                n.enabled,
                n.actions.len()
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "nickname",
                AccessibleRole::TextInput,
                "Surnom",
                "Ada",
                true,
                true,
                2
            ),
            ("play", AccessibleRole::Button, "Jouer", "", false, true, 1),
            (
                "skin",
                AccessibleRole::Button,
                "Changer d'apparence",
                "",
                false,
                true,
                1
            ),
            (
                "server",
                AccessibleRole::Button,
                "Choisir un serveur",
                "",
                false,
                true,
                1
            ),
            (
                "quality",
                AccessibleRole::Button,
                "Qualité graphique",
                "Élevée",
                false,
                true,
                1
            ),
        ]
    );
    assert_eq!(
        nodes[0].actions,
        vec![
            AccessibleAction::FocusNickname,
            AccessibleAction::EditNickname
        ]
    );
    assert_eq!(
        nodes[1].actions,
        vec![AccessibleAction::Menu(MenuAction::Play)]
    );
    menu.action(MenuAction::Play, true);
    let busy = login_accessibility(&menu.view(true), &input);
    assert!(busy[..5].iter().all(|n| !n.enabled && !n.focused));
    assert_eq!(
        busy[5],
        AccessibleNode {
            id: "status",
            role: AccessibleRole::Status,
            name: "Connexion en cours".into(),
            value: "".into(),
            focused: false,
            enabled: true,
            actions: vec![]
        }
    );
    menu.step(8., false).unwrap();
    assert!(menu.begin_outgoing());
    menu.step(600., false).unwrap();
    assert!(login_accessibility(&menu.view(true), &input).is_empty());
}

fn assert_accessible_locale(
    locale: MenuLocale,
    names: [&str; 3],
    values: [&str; 2],
    statuses: [&str; 2],
) {
    // Authored expected wording, tested independently of the wording table.
    for (quality, expected_value) in [
        (MenuQuality::High, values[0]),
        (MenuQuality::Low, values[1]),
    ] {
        let menu = StartMenuState::new(locale, false, quality);
        let nodes = login_accessibility(&menu.view(true), &MenuInputState::default());
        for (id, expected_name) in ["skin", "server", "quality"].into_iter().zip(names) {
            let node = nodes.iter().find(|node| node.id == id).unwrap();
            assert_eq!(node.role, AccessibleRole::Button);
            assert_eq!(node.name, expected_name);
            assert_eq!(
                node.value,
                if id == "quality" { expected_value } else { "" }
            );
        }
    }
    let mut menu = StartMenuState::new(locale, false, MenuQuality::High);
    menu.action(MenuAction::Play, true);
    let status = |menu: &StartMenuState| {
        login_accessibility(&menu.view(true), &MenuInputState::default())
            .into_iter()
            .find(|node| node.id == "status")
            .unwrap()
    };
    assert_eq!(status(&menu).name, statuses[0]);
    menu.step(0., false).unwrap();
    assert!(menu.begin_outgoing());
    assert!(
        menu.accepted_game_over(slither_client::session_policy::Phase::Victory, None)
            .unwrap()
    );
    assert_eq!(status(&menu).name, statuses[1]);
}

#[test]
fn english_accessible_button_names_and_quality_values() {
    assert_accessible_locale(
        MenuLocale::English,
        ["Change Skin", "Choose Server", "Graphics quality"],
        ["High", "Low"],
        ["Connecting", "Champion message handoff pending"],
    );
}

#[test]
fn german_accessible_button_names_and_quality_values() {
    assert_accessible_locale(
        MenuLocale::German,
        ["Skin ändern", "Server wählen", "Grafikqualität"],
        ["Hoch", "Niedrig"],
        [
            "Verbindung wird hergestellt",
            "Übermittlung der Champion-Nachricht ausstehend",
        ],
    );
}

#[test]
fn french_accessible_button_names_and_quality_values() {
    assert_accessible_locale(
        MenuLocale::French,
        [
            "Changer d'apparence",
            "Choisir un serveur",
            "Qualité graphique",
        ],
        ["Élevée", "Faible"],
        [
            "Connexion en cours",
            "Transmission du message du champion en attente",
        ],
    );
}

#[test]
fn portuguese_accessible_button_names_and_quality_values() {
    assert_accessible_locale(
        MenuLocale::Portuguese,
        ["Mudar aparência", "Escolher servidor", "Qualidade gráfica"],
        ["Alta", "Baixa"],
        ["Conectando", "Envio da mensagem do campeão pendente"],
    );
}
#[test]
fn caret_clock_restarts_on_focus_edit_move_and_preedit() {
    use slither_client::menu_input::{EditIntent as I, MenuInputState, Movement, TextField};
    let mut input = MenuInputState::default();
    input.apply(I::Focus(Some(TextField::Nickname)));
    for (now, visible) in [
        (0., true),
        (529., true),
        (530., false),
        (1059., false),
        (1060., true),
    ] {
        input.step_caret(now);
        assert_eq!(input.caret_visible(), visible);
    }
    input.step_caret(1600.);
    assert!(!input.caret_visible());
    for intent in [
        I::KeyText("Player".into()),
        I::Move {
            movement: Movement::Left,
            extend: false,
        },
        I::Preedit("é".into(), Some((0, 2))),
    ] {
        input.apply(intent);
        assert!(input.caret_visible());
        input.step_caret(2140.);
    }
    input.apply(I::WindowFocused(false));
    assert!(!input.caret_visible());
    input.apply(I::WindowFocused(true));
    assert!(input.caret_visible());
    input.step_caret(f64::NAN);
    input.step_caret(0.);
    assert!(input.caret_visible());
    input.apply(I::Focus(None));
    assert!(!input.caret_visible());
}

#[test]
fn caret_edit_after_idle_uses_the_next_menu_clock_seam() {
    let mut input = MenuInputState::default();
    input.apply(EditIntent::Focus(Some(TextField::Nickname)));
    input.step_caret(0.);
    input.step_caret(600.);
    assert!(!input.caret_visible());
    input.apply(EditIntent::KeyText("Player".into()));
    assert!(input.caret_visible());
    input.step_caret(10_000.);
    assert!(input.caret_visible());
    input.step_caret(10_529.);
    assert!(input.caret_visible());
    input.step_caret(10_530.);
    assert!(!input.caret_visible());
}
