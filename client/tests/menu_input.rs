//! Authored synthetic editor/event traces; no OS input, clipboard or window.
use slither_client::{
    desktop_input::{DesktopInput, OwnInput},
    menu_input::*,
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
fn normalization_is_identical_for_typing_paste_and_ime_commit() {
    for field in [TextField::Nickname, TextField::Champion] {
        let raw = "x\0\u{7f}é😀e\u{301}界".repeat(30);
        let typed = editor(field, &raw);
        let mut pasted = editor(field, "");
        assert!(paste(&mut pasted, Ok(raw.clone())).is_empty());
        let mut ime = editor(field, "");
        ime.apply(EditIntent::Preedit(raw.clone(), None));
        assert_eq!(text(&ime, field), "");
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
        assert_eq!(text(&ime, field), "");
        ime.apply(EditIntent::ImeCommit(raw));
        assert_eq!(text(&ime, field), expected);
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
}

#[test]
fn raw_preedit_commit_and_composition_enter_are_separate() {
    let field = TextField::Champion;
    let mut input = editor(field, "old");
    input.apply(EditIntent::SelectAll);
    input.apply(EditIntent::ImeEnabled(true));
    input.apply(EditIntent::Preedit("界😀".into(), Some((3, 7))));
    // Defensive out-of-contract keyboard text must not touch the raw preview.
    input.apply(EditIntent::KeyText("unexpected".into()));
    assert_eq!(text(&input, field), "old");
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
    input.apply(EditIntent::Move {
        movement: Movement::Left,
        extend: false,
    });
    input.apply(EditIntent::Move {
        movement: Movement::End,
        extend: true,
    });
    input.apply(EditIntent::KeyText("new".into()));
    assert_eq!(text(&input, field), "new");
    input.apply(EditIntent::Undo);
    input.apply(EditIntent::Redo);
    assert_eq!(text(&input, field), "new");
    input.apply(EditIntent::Backspace);
    assert_eq!(text(&input, field), "ne");
    input.apply(EditIntent::Undo);
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
    input.apply(EditIntent::Move {
        movement: Movement::WordLeft,
        extend: false,
    });
    input.apply(EditIntent::Move {
        movement: Movement::WordRight,
        extend: false,
    });
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
fn preedit_is_bounded_on_scalar_boundaries() {
    let preview = bounded_preedit(&"界".repeat(100_000));
    assert_eq!(preview, "界".repeat(MAX_PREEDIT_UNITS));
    assert!(preview.capacity() <= 3 * MAX_PREEDIT_UNITS);
    for prefix in [MAX_PREEDIT_UNITS - 2, MAX_PREEDIT_UNITS - 1] {
        let preview = bounded_preedit(&("x".repeat(prefix) + "😀tail"));
        assert!(preview.encode_utf16().count() <= MAX_PREEDIT_UNITS);
        assert_eq!(preview.ends_with('😀'), prefix == MAX_PREEDIT_UNITS - 2);
    }
}
