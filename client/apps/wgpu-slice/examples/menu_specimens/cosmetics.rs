//! Authored-art offscreen studio specimens and the complete menu-record ledger.
use super::*;
use slither_client::{menu_controls::ControlId, skin_studio::*};
use slither_wgpu_slice::{
    login_native::NativeLogin, menu_counts::MenuComponent, menu_storage::MenuStorage,
};

fn capture_studio(renderer: &mut Renderer, menu: &MenuFrame, file: &Path) -> Result<()> {
    let mut frame = menu.frame.clone();
    if let Some(cover) = &menu.world_cover {
        frame.draws.insert(0, cover.clone());
    }
    if let Some(page) = &menu.backdrop {
        frame.draws.insert(0, page.clone());
    }
    let pixels = readback(renderer, &frame, &menu.resources, None)?;
    use image::{
        ImageEncoder,
        codecs::png::{CompressionType, FilterType, PngEncoder},
    };
    PngEncoder::new_with_quality(
        std::fs::File::create(file).map_err(|e| e.to_string())?,
        CompressionType::Fast,
        FilterType::NoFilter,
    )
    .write_image(
        &pixels,
        frame.extent[0],
        frame.extent[1],
        image::ExtendedColorType::Rgba8,
    )
    .map_err(|e| e.to_string())
}
pub async fn specimens(renderer: &mut Renderer, out: &Path, directory: &Path) -> Result<()> {
    let mut ready = Cosmetics::directory(directory, None)?;
    let mut missing = Cosmetics::from_images(Images::default(), None)?;
    let validation = renderer
        .device
        .push_error_scope(wgpu::ErrorFilter::Validation);
    let mut ledger = String::from(
        "case,dpr,menu_reported,menu_admitted,studio_section,studio_chrome,slots,bytes,preview,combined\n",
    );
    let mut peak = 0;
    for dpr in [1., 1.25, 1.5, 1.75, 2., 3., 4., 5., 6., 7., 8.] {
        for (layout, logical) in [
            ("wide", [960., 540.]),
            ("narrow", [360., 640.]),
            ("short", [960., 360.]),
        ] {
            let extent = logical.map(|v| (v * dpr) as u32);
            for available in [true, false] {
                renderer.sync_resources(&[])?;
                let art = if available { &mut ready } else { &mut missing };
                let label = if available { "art" } else { "missing" };
                let mut login = NativeLogin::with_preferences(
                    AppearanceRecord {
                        builtin: 7,
                        ..Default::default()
                    },
                    MenuStorage::at(None),
                )?;
                for now in (0..=9600).step_by(16) {
                    login.step(now as f64, false, false)?;
                }
                login.prepare_host(extent, dpr, false, Some(art))?;
                login.open_studio();
                login.studio.activate(COSMETICS_TAB, "Player");
                for time in [
                    0., 1., 90., 115., 180., 229., 230., 320., 410., 525., 630., 640., 755., 870.,
                ] {
                    login.step(9600. + time, false, false)?;
                    match time as u32 {
                        115 => login.studio.state.select_cosmetic(Some(8)),
                        230 => {
                            login.studio.activate(COSMETIC_NEXT, "Player");
                        }
                        320 => {
                            login.studio.activate(BUILDER_TAB, "Player");
                            login.studio.state.edit_builder(|b| {
                                b.append(1);
                            });
                        }
                        410 => login.studio.activate(COSMETICS_TAB, "Player"),
                        525 => {
                            login.studio.state.select_cosmetic(Some(31));
                            login.controls.set_focus(ControlId(COSMETIC_BASE + 25));
                            login.controls.set_progress(COSMETICS_TAB, [1., 1.]);
                            login.controls.set_progress(SAVE, [1., 1.]);
                        }
                        640 => login
                            .studio
                            .activate_with_random(SAVE, "Player", &mut || 17),
                        _ => {}
                    }
                    if [1., 525.].contains(&time) {
                        for control in &login.panel.controls {
                            if control.enabled {
                                login.controls.set_progress(control.id, [1., 1.]);
                            }
                        }
                    }
                    login.studio.sync_art(logical, Some(art))?;
                    let mut menu = login
                        .prepare_host(extent, dpr, false, Some(art))
                        .map_err(|e| format!("{layout} {label} {dpr} {time}: {e}"))?;
                    let slots = menu.resources.len();
                    let bytes: usize = menu.resources.iter().map(|r| r.image.pixels.len()).sum();
                    if slots > 64 || bytes > 13 * 1024 * 1024 {
                        return Err("Cosmetics menu resource allowance".into());
                    }
                    let preview = if login.studio.state.visible() {
                        let (frame, resources) =
                            login.studio.prepare_preview(extent, dpr, Some(art))?;
                        let count = frame.draws.len();
                        menu.frame.draws.extend(frame.draws);
                        menu.resources.extend(resources);
                        count
                    } else {
                        0
                    };
                    let counts = menu.draw_counts(preview);
                    let section = counts.components.get(MenuComponent::StudioSection);
                    if counts.components.total() + preview != menu.frame.draws.len()
                        || counts.admitted_menu() > 448
                        || section > 88
                        || preview > 1024
                        || counts.combined() > 1536
                    {
                        return Err(format!(
                            "Cosmetics allowance {layout} {label} DPR{dpr} t{time}: {counts:?}"
                        ));
                    }
                    peak = peak.max(counts.admitted_menu());
                    let case = format!("{layout}-{label}-{time:03}");
                    ledger.push_str(&format!(
                        "{case},{dpr},{},{},{section},{},{slots},{bytes},{preview},{}\n",
                        counts.reported_menu(),
                        counts.admitted_menu(),
                        counts.components.get(MenuComponent::StudioChrome),
                        counts.combined()
                    ));
                    if [1., 1.5, 2.].contains(&dpr)
                        && [115., 180., 230., 525., 630., 755.].contains(&time)
                    {
                        capture_studio(
                            renderer,
                            &menu,
                            &out.join(format!("cosmetics-{case}-{dpr}x.png")),
                        )?;
                    }
                    login.studio.presented();
                }
                if [1., 1.5, 2.].contains(&dpr) && available && layout == "wide" {
                    let saved = login.studio.state.committed().clone();
                    let parsed =
                        AppearanceRecord::parse(&saved.serialize().map_err(str::to_owned)?)
                            .ok_or("Saved specimen record")?;
                    assert_eq!(saved, parsed);
                    let mut restored =
                        NativeLogin::with_preferences(parsed, MenuStorage::at(None))?;
                    renderer.sync_resources(&[])?;
                    for now in (0..=9600).step_by(16) {
                        restored.step(now as f64, false, false)?;
                    }
                    restored.prepare_host(extent, dpr, false, Some(art))?;
                    restored.open_studio();
                    restored.step(9830., false, false)?;
                    restored.studio.sync_art(logical, Some(art))?;
                    let mut menu = restored.prepare_host(extent, dpr, false, Some(art))?;
                    let (preview, resources) =
                        restored.studio.prepare_preview(extent, dpr, Some(art))?;
                    menu.frame.draws.extend(preview.draws);
                    menu.resources.extend(resources);
                    capture_studio(
                        renderer,
                        &menu,
                        &out.join(format!("cosmetics-save-restore-{dpr}x.png")),
                    )?;
                }
            }
        }
        std::fs::write(out.join("dpr-ledger.csv"), &ledger).map_err(|e| e.to_string())?;
        println!("Cosmetics DPR {dpr}, peak {peak}/448");
    }
    if let Some(error) = validation.pop().await {
        return Err(error.to_string());
    }
    println!(
        "Completed {} cases, peak {peak}/448",
        ledger.lines().count() - 1
    );
    Ok(())
}
