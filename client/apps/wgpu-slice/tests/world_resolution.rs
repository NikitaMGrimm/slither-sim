//! Explicit finite offscreen checks; no windows, sockets or external artwork.
#![cfg(not(target_arch = "wasm32"))]
#[path = "../examples/support/offscreen.rs"]
mod offscreen;
use slither_client::settings::WorldResolution::{Native, Original};
use slither_wgpu_slice::{
    gpu::{Renderer, instance},
    world_presenter::WorldPresenter,
};

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
                let physical = [4096, 2160]; // MSAA4 exceeds the unchanged guard
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
                assert_eq!(report.plan.effective, Original);
                assert!(report.plan.native_unavailable);
                assert_eq!(p.backing_extent(), Some(world.extent));
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
