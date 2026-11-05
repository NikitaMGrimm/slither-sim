//! Authored disconnect through the production native compositor, entirely offscreen.
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
    let mut renderer = Renderer::new(&instance(), None, wgpu::TextureFormat::Rgba8Unorm).await?;
    let mut presenter = WorldPresenter::new(&renderer);
    let mut game = tape::playing();
    let mut login = NativeLogin::new("Player")?;
    login.step(0., true, false)?;
    login.activate(true);
    login.step(16., true, false)?;
    let physical = [3840, 2160];
    let dpr = 2.;
    let window = WorldWindow {
        physical,
        dpr,
        requested: WorldResolution::Original,
    };
    let extent = window.viewport(&mut game)?;
    let mut board = vec![b'l', 1, 0, 2, 0, 10];
    for i in 0..10 {
        let name = format!("Bot {:02}", i + 1);
        board.extend([0, 12, 0, 0, 0, 41, name.len() as u8]);
        board.extend(name.bytes());
    }
    game.receive(game.generation(), &tape::bundle(&[board]), 30.)?;
    for now in (32..=1008).step_by(16) {
        game.receive(
            game.generation(),
            &tape::bundle(&[vec![b'p']]),
            f64::from(now),
        )?;
        game.begin_redraw(f64::from(now), extent, 1.)?;
        tape::effects(&mut game);
        login.observe_game(&game)?;
        login.step(
            f64::from(now),
            game.can_play(),
            game.phase() == Phase::Playing,
        )?;
        let picture = game.take_frame().ok_or("Missing warm picture")?;
        game.presented(true)?;
        game.bank.recycle_frame(picture);
    }
    game.transport_failed(
        game.generation(),
        "IO error: connection reset by remote host (os error 10054)",
    )?;
    tape::effects(&mut game);
    for now in [1024., 1040., 1056.] {
        login.observe_game(&game)?;
        login.step(now, game.can_play(), false)?;
        game.begin_redraw(now, extent, 1.)?;
        tape::effects(&mut game);
        let picture = game.take_frame().ok_or("Missing failed picture")?;
        if now == 1056. {
            let menu = login.prepare_host(physical, dpr, game.can_play(), None)?;
            let composed = menu.compose_over(&picture)?;
            renderer.sync_resources(&composed.resources)?;
            let target = offscreen::target(&renderer, physical);
            let view = target.create_view(&Default::default());
            presenter.render(
                &mut renderer,
                &composed.frame,
                composed.overlay.as_ref(),
                &view,
                physical,
                WorldResolution::Original,
            )?;
            login.egui.paint(&renderer, &view);
            let pixels = offscreen::pixels(&renderer, &target)?;
            image::save_buffer(
                output.join("disconnect-4k.png"),
                &pixels,
                physical[0],
                physical[1],
                image::ColorType::Rgba8,
            )
            .map_err(|e| e.to_string())?;
            std::fs::write(output.join("state.txt"), format!("Authored in-memory packets; no sockets, windows or external assets.\nPhysical {physical:?}; DPR {dpr}; time {now}; phase {:?}; menu {:?}; world draws {}; overlay draws {}\n", game.phase(), login.state.view(game.can_play()).phase, picture.frame.draws.len(), picture.overlay.as_ref().map_or(0, |f| f.draws.len()))).map_err(|e|e.to_string())?;
        }
        game.presented(true)?;
        game.bank.recycle_frame(picture);
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
