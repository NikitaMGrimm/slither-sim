//! Explicit finite offscreen checks; no windows, sockets or external artwork.
#![cfg(not(target_arch = "wasm32"))]
#[path = "../examples/support/offscreen.rs"]
mod offscreen;
#[cfg(windows)]
#[path = "support/announcement_tape.rs"]
mod tape;
use slither_client::settings::WorldResolution::{Native, Original};
use slither_wgpu_slice::{
    gpu::{Renderer, instance},
    world_presenter::WorldPresenter,
};

#[cfg(windows)]
#[test]
#[ignore = "explicit finite offscreen GPU check"]
fn production_window_modes_preserve_generated_frames_vertices_and_input_packets() {
    use slither_wgpu_slice::world_resolution::WorldWindow;
    let mut renderer = pollster::block_on(Renderer::new(
        &instance(),
        None,
        wgpu::TextureFormat::Rgba8Unorm,
    ))
    .unwrap();
    let scope = renderer
        .device
        .push_error_scope(wgpu::ErrorFilter::Validation);
    let mut presenter = WorldPresenter::new(&renderer);
    for dpr in [1., 1.25, 1.5, 2.] {
        let mut original = None;
        for requested in [Original, Native] {
            let window = WorldWindow {
                physical: [1920, 1080],
                dpr,
                requested,
            };
            let mut game = tape::playing();
            let extent = window.viewport(&mut game).unwrap();
            let css = window.css_size();
            assert_eq!(css, window.physical.map(|n| f64::from(n) / dpr));
            window
                .pointer(&mut game, [css[0] * 0.86, css[1] * 0.76], false)
                .unwrap();
            game.key("Space", true);
            game.begin_redraw(300., extent, 1.).unwrap();
            let on = tape::effects(&mut game);
            assert!(on.contains(&vec![253]));
            assert!(
                on.iter().any(|p| p.len() == 1 && p[0] < 251),
                "steering angle required"
            );
            let picture = game.take_frame().unwrap();
            assert_eq!(
                picture.frame.extent,
                slither_wgpu_slice::render::backing_size(css).unwrap(),
                "{requested:?} projection at DPR {dpr}"
            );
            // Each independent game fixture owns a fresh resource-handle namespace.
            renderer.sync_resources(&[]).unwrap();
            renderer.sync_resources(&picture.resources).unwrap();
            let target = offscreen::target(&renderer, window.physical);
            let report = presenter
                .render(
                    &mut renderer,
                    &picture.frame,
                    picture.overlay.as_ref(),
                    &target.create_view(&Default::default()),
                    window.physical,
                    window.requested,
                )
                .unwrap();
            assert_eq!(report.plan.effective, requested);
            assert!(report.world_draws > 0);
            assert_eq!(report.plan.blit, requested == Original);
            assert_eq!(presenter.backing_extent().is_some(), requested == Original);
            let resident = picture
                .resources
                .iter()
                .map(|r| (r.asset.into(), r.asset))
                .collect();
            let bits = picture
                .frame
                .quads(&resident)
                .unwrap()
                .into_iter()
                .map(|q| q.vertices.map(|v| v.map(f32::to_bits)))
                .collect::<Vec<_>>();
            game.presented(true).unwrap();
            game.key("Space", false);
            let extent = window.viewport(&mut game).unwrap();
            game.begin_redraw(360., extent, 1.).unwrap();
            let off = tape::effects(&mut game);
            assert!(off.contains(&vec![254]));
            let snapshot = (picture.frame, picture.overlay, bits, on, off);
            if let Some(original) = &original {
                assert_eq!(&snapshot, original, "mode parity at DPR {dpr}");
            } else {
                original = Some(snapshot);
            }
            offscreen::pixels(&renderer, &target).unwrap();
        }
    }
    assert!(pollster::block_on(scope.pop()).is_none());
}

#[test]
#[ignore = "explicit finite offscreen GPU check"]
fn direct_resolve_sample_one_resize_and_retained_submissions() {
    pollster::block_on(async {
        for samples in [1, 4] {
            let adapter = instance()
                .request_adapter(&Default::default())
                .await
                .unwrap();
            let mut r = Renderer::from_adapter_with_max_samples(
                adapter,
                wgpu::TextureFormat::Rgba8Unorm,
                samples,
            )
            .await
            .unwrap();
            assert_eq!(r.sample_count, samples);
            let scope = r.device.push_error_scope(wgpu::ErrorFilter::Validation);
            let (world, _, resources) = offscreen::scene([128, 128]);
            r.sync_resources(&resources).unwrap();
            let mut p = WorldPresenter::new(&r);
            let mut retained = Vec::new();
            // No waits: each output remains alive while old world targets retire.
            for (mode, physical) in [
                (Original, [128, 128]),
                (Native, [128, 128]),
                (Native, [192, 96]),
                (Original, [128, 128]),
                (Native, [128, 128]),
            ] {
                let target = offscreen::target(&r, physical);
                let (_, overlay, _) = offscreen::scene(physical);
                let frame = world.clone();
                let report = p
                    .render(
                        &mut r,
                        &frame,
                        Some(&overlay),
                        &target.create_view(&Default::default()),
                        physical,
                        mode,
                    )
                    .unwrap();
                assert_eq!(frame, world);
                assert_eq!(
                    report.plan.attachment,
                    if mode == Native {
                        physical
                    } else {
                        world.extent
                    }
                );
                assert_eq!(report.plan.blit, mode == Original);
                assert_eq!(report.passes, if mode == Native { 2 } else { 3 });
                assert_eq!(
                    report.target_bytes,
                    report.plan.msaa_bytes() + report.plan.backing_bytes()
                );
                assert_eq!(
                    p.backing_extent(),
                    if mode == Native {
                        None
                    } else {
                        Some(world.extent)
                    }
                );
                retained.push((target, report));
            }
            let before = p.effective();
            let msaa = r.multisample_extent();
            let (target, _) = retained.last().unwrap();
            let (_, mut invalid, _) = offscreen::scene([128, 128]);
            invalid.draws[0].effective_alpha = f64::NAN;
            assert!(
                p.render(
                    &mut r,
                    &world,
                    Some(&invalid),
                    &target.create_view(&Default::default()),
                    [128, 128],
                    Original
                )
                .is_err()
            );
            assert_eq!(p.effective(), before);
            assert_eq!(r.multisample_extent(), msaa);
            let mut image_pairs = Vec::new();
            for (target, _) in retained {
                let pixels = offscreen::pixels(&r, &target).unwrap();
                assert_eq!(
                    &pixels[(20 * target.width() as usize + 32) * 4..][..4],
                    &[78, 180, 153, 255]
                );
                if target.width() == 128 {
                    image_pairs.push(pixels);
                }
            }
            assert_eq!(image_pairs[0], image_pairs[2]); // Original after swap
            assert_eq!(image_pairs[1], image_pairs[3]); // Native after resize/swap
            if samples == 4 {
                let physical = [4096, 2160]; // Keep Native sharp without a large MSAA target.
                let target = offscreen::target(&r, physical);
                let (_, overlay, _) = offscreen::scene(physical);
                let report = p
                    .render(
                        &mut r,
                        &world,
                        Some(&overlay),
                        &target.create_view(&Default::default()),
                        physical,
                        Native,
                    )
                    .unwrap();
                assert_eq!(report.plan.effective, Native);
                assert_eq!(report.plan.samples, 1);
                assert!(!report.plan.native_unavailable);
                assert_eq!(report.target_bytes, 0);
                assert!(r.multisample_extent().is_none());
                assert!(p.backing_extent().is_none());
                let target = offscreen::target(&r, [128, 128]);
                let (_, overlay, _) = offscreen::scene([128, 128]);
                assert_eq!(
                    p.render(
                        &mut r,
                        &world,
                        Some(&overlay),
                        &target.create_view(&Default::default()),
                        [128, 128],
                        Native
                    )
                    .unwrap()
                    .plan
                    .effective,
                    Native
                );
                assert!(p.backing_extent().is_none());
            }
            assert!(scope.pop().await.is_none());
        }
    });
}
