//! Authored in-game menu over a live tape world, entirely offscreen.
#[cfg(windows)]
#[allow(dead_code)]
#[path = "support/offscreen.rs"]
mod offscreen;
#[cfg(windows)]
#[path = "../tests/support/announcement_tape.rs"]
mod tape;

#[cfg(windows)]
async fn capture(output: &std::path::Path) -> slither_wgpu_slice::Result<()> {
    use slither_client::{session_policy::Phase, settings::WorldResolution};
    use slither_wgpu_slice::{
        gpu::{Renderer, instance},
        login_native::NativeLogin,
        world_presenter::WorldPresenter,
        world_resolution::WorldWindow,
    };
    std::fs::create_dir(output).map_err(|e| e.to_string())?;
    let mut ledger = String::from(
        "Authored in-memory packets; no sockets, windows, art directories or external assets.\n",
    );
    for (size, physical, dpr) in [("1080p", [1920, 1080], 1.), ("4k", [3840, 2160], 2.)] {
        for (mode, requested) in [
            ("original", WorldResolution::Original),
            ("native", WorldResolution::Native),
        ] {
            for screen in SCREENS {
                // Each authored game owns its resource handles; renderers are not shared.
                let mut renderer =
                    Renderer::new(&instance(), None, wgpu::TextureFormat::Rgba8Unorm).await?;
                let mut presenter = WorldPresenter::new(&renderer);
                let mut game = tape::playing();
                let mut login = NativeLogin::new("Player")?;
                // Settings shows the same world resolution the picture uses.
                login.state.settings = slither_client::settings::SettingsState::new(
                    slither_client::settings::SettingsPreferences::parse(match requested {
                        WorldResolution::Original => "2:1:original",
                        WorldResolution::Native => "2:1:native",
                    })
                    .ok_or("Invalid authored settings record")?,
                );
                login.step(0., true, false)?;
                login.activate(true);
                login.step(16., true, false)?;
                let window = WorldWindow {
                    physical,
                    dpr,
                    requested,
                };
                let extent = window.viewport(&mut game)?;
                let mut board = vec![b'l', 1, 0, 2, 0, 10];
                for i in 0..10 {
                    let name = format!("Bot {:02}", i + 1);
                    board.extend([0, 12, 0, 0, 0, 41, name.len() as u8]);
                    board.extend(name.bytes());
                }
                game.receive(game.generation(), &tape::bundle(&[board]), 30.)?;
                let mut now = 32.;
                let frame = |game: &mut slither_wgpu_slice::game::LocalGame,
                             login: &mut NativeLogin,
                             now: f64|
                 -> slither_wgpu_slice::Result<_> {
                    game.receive(game.generation(), &tape::bundle(&[vec![b'p']]), now)?;
                    game.begin_redraw(now, extent, 1.)?;
                    tape::effects(game);
                    login.observe_game(game)?;
                    login.step(now, game.can_play(), game.phase() == Phase::Playing)?;
                    login.prepare_ingame(game, physical, dpr)?;
                    game.take_frame()
                        .ok_or_else(|| "Missing picture".to_string())
                };
                while now <= 1008. {
                    let picture = frame(&mut game, &mut login, now)?;
                    game.presented(true)?;
                    game.bank.recycle_frame(picture);
                    now += 16.;
                }
                open(screen, &mut game, &mut login, now)?;
                // Settle the entry fades on the same clock as the desktop host.
                for _ in 0..40 {
                    now += 16.;
                    let picture = frame(&mut game, &mut login, now)?;
                    game.presented(true)?;
                    game.bank.recycle_frame(picture);
                }
                now += 16.;
                let picture = frame(&mut game, &mut login, now)?;
                renderer.sync_resources(&picture.resources)?;
                let target = offscreen::target(&renderer, physical);
                let view = target.create_view(&Default::default());
                presenter.render(
                    &mut renderer,
                    &picture.frame,
                    picture.overlay.as_ref(),
                    &view,
                    physical,
                    requested,
                )?;
                login.egui.paint(&renderer, &view);
                let pixels = offscreen::pixels(&renderer, &target)?;
                let name = format!("{screen}-{size}-{mode}.png");
                image::save_buffer(
                    output.join(&name),
                    &pixels,
                    physical[0],
                    physical[1],
                    image::ColorType::Rgba8,
                )
                .map_err(|e| e.to_string())?;
                ledger.push_str(&format!(
                    "{name}: physical {physical:?}; DPR {dpr}; time {now}; phase {:?}; menu open {}; world draws {}; HUD overlay draws {}\n",
                    game.phase(),
                    game.ingame_menu_open(),
                    picture.frame.draws.len(),
                    picture.overlay.as_ref().map_or(0, |f| f.draws.len()),
                ));
                game.presented(true)?;
                game.bank.recycle_frame(picture);
            }
        }
    }
    std::fs::write(output.join("state.txt"), ledger).map_err(|e| e.to_string())
}

/// `playing` is the live HUD control; the menu states must draw none of it.
#[cfg(windows)]
const SCREENS: [&str; 3] = ["playing", "menu", "settings"];

#[cfg(windows)]
fn open(
    screen: &str,
    game: &mut slither_wgpu_slice::game::LocalGame,
    login: &mut slither_wgpu_slice::login_native::NativeLogin,
    now: f64,
) -> slither_wgpu_slice::Result<()> {
    if screen == "playing" {
        return Ok(());
    }
    if !login.open_ingame_menu(game, now)? {
        return Err("In-game menu did not open".into());
    }
    if screen == "settings" {
        let key = |key, pressed| egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        login.ingame_input(
            game,
            egui::RawInput {
                events: vec![
                    key(egui::Key::Tab, true),
                    key(egui::Key::Tab, false),
                    key(egui::Key::Enter, true),
                    key(egui::Key::Enter, false),
                ],
                ..Default::default()
            },
        )?;
        if !login.state.settings.is_open() {
            return Err("In-game Settings did not open".into());
        }
    }
    Ok(())
}

fn main() {
    #[cfg(windows)]
    {
        let output = std::env::args_os().nth(1).expect("output directory");
        pollster::block_on(capture(std::path::Path::new(&output))).unwrap();
    }
    #[cfg(not(windows))]
    panic!("native offscreen menu evidence requires Windows");
}
