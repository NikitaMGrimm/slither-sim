//! Finite champion editor specimens through the production login host, from an
//! authored tape victory. No window, socket or art.
use super::*;
use slither_client::champion::{MESSAGE, SAVE};
use slither_wgpu_slice::{champion_egui::widget_id, game::LocalGame, login_native::NativeLogin};

#[path = "../../tests/support/announcement_tape.rs"]
mod tape;

/// A committed victory replayed from the authored tape, with the login fully
/// returned below the editor.
fn editor() -> Result<(LocalGame, NativeLogin)> {
    let mut game = tape::playing();
    let mut login = NativeLogin::new("Player")?;
    login.step(0., true, false)?;
    login.activate(true);
    login.step(8., true, false)?;
    tape::pump(&mut game, 700.);
    login.observe_game(&game)?;
    login.step(700., false, true)?;
    game.receive(game.generation(), &[b'v', 1], 800.)?;
    for now in (808..=9608).step_by(40) {
        game.receive(game.generation(), b"p", f64::from(now))?;
        tape::pump(&mut game, f64::from(now));
        login.observe_game(&game)?;
        login.step(f64::from(now), false, false)?;
    }
    // The caret's phase would otherwise follow the wall clock.
    if let Some(ctx) = login.egui.context() {
        ctx.all_styles_mut(|style| style.visuals.text_cursor.blink = false);
    }
    Ok((game, login))
}
fn shot(
    renderer: &mut Renderer,
    login: &mut NativeLogin,
    output: &Path,
    name: &str,
    extent: [u32; 2],
    dpr: f64,
) -> Result<serde_json::Value> {
    login.prepare_host(extent, dpr, true, None)?;
    let picture = login.prepare_host(extent, dpr, true, None)?;
    let shapes = login.egui.prepared_shapes().len();
    capture_settings(
        renderer,
        &picture,
        login,
        &output.join(format!("{name}-scale-{dpr}.png")),
    )?;
    Ok(serde_json::json!({"state": name, "extent": extent, "dpr": dpr, "egui_shapes": shapes}))
}
pub fn specimens(renderer: &mut Renderer, output: &Path) -> Result<()> {
    let mut rows = Vec::new();
    for dpr in [1., 1.5, 2.] {
        let extent = [(960. * dpr) as u32, (720. * dpr) as u32];
        let (_game, mut login) = editor()?;
        login.champion.ui.message = "Thanks for a great game!".into();
        rows.push(shot(renderer, &mut login, output, "champion", extent, dpr)?);
        login.champion.ui.message = "Wide message ".repeat(10);
        rows.push(shot(
            renderer,
            &mut login,
            output,
            "champion-scroll",
            extent,
            dpr,
        )?);
        login.champion.ui.keyboard_focus = true;
        login.egui.focus(widget_id(SAVE));
        rows.push(shot(
            renderer,
            &mut login,
            output,
            "champion-save-focus",
            extent,
            dpr,
        )?);
        login.egui.focus(widget_id(MESSAGE));
        rows.push(shot(
            renderer,
            &mut login,
            output,
            "champion-field-focus",
            extent,
            dpr,
        )?);
    }
    std::fs::write(
        output.join("champion.json"),
        serde_json::to_vec_pretty(&rows).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
