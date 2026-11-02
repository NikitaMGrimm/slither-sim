//! Authored offline edit/save/restore fixtures, with injected header entropy.
use super::*;
use slither_client::{skin_builder::PALETTES, skin_studio::*};
use slither_wgpu_slice::{login_native::NativeLogin, menu_storage::MenuStorage};

fn opened(record: AppearanceRecord, storage: MenuStorage) -> Result<NativeLogin> {
    let mut login = NativeLogin::with_preferences(record, storage)?;
    for now in (0..=9600).step_by(16) {
        login.step(f64::from(now), false, false)?;
    }
    login.open_studio();
    login.step(9830., false, false)?;
    login.studio.activate(BUILDER_TAB, "Builder");
    login.step(10010., false, false)?;
    Ok(login)
}
fn shot(
    renderer: &mut Renderer,
    login: &mut NativeLogin,
    logical: [f64; 2],
    dpr: f64,
    path: &Path,
) -> Result<()> {
    let extent = logical.map(|n| (n * dpr) as u32);
    let (picture, _) = studio_picture(login, extent, dpr, false, None)?;
    capture_layered(renderer, &picture, login, path)
}
fn record_ledger(
    login: &mut NativeLogin,
    dpr: f64,
    name: &str,
    ledger: &mut Vec<String>,
) -> Result<()> {
    for logical in [[960., 540.], [360., 640.], [960., 360.]] {
        let extent = logical.map(|n| (n * dpr) as u32);
        // The egui studio's controls are shapes in the shared egui pass; the
        // menu picture holds only the page and the preview.
        let (picture, [preview, slots, bytes]) = studio_picture(login, extent, dpr, false, None)?;
        if preview > 1024 || picture.frame.draws.len() > preview {
            return Err(format!(
                "Builder working envelope exceeded: {dpr} {name} {logical:?}"
            ));
        }
        ledger.push(format!(
            "{dpr},{name},{},{},{},{preview},{slots},{bytes}",
            logical[0],
            logical[1],
            login.egui.prepared_shapes().len(),
        ));
    }
    Ok(())
}
pub fn specimens(renderer: &mut Renderer, output: &Path) -> Result<()> {
    for dpr in [1., 1.5, 2.] {
        renderer.sync_resources(&[])?;
        let directory = output.join(format!("preferences-{dpr}x"));
        let mut login = opened(
            AppearanceRecord {
                nickname: "Builder".into(),
                builtin: 7,
                ..Default::default()
            },
            MenuStorage::at(Some(directory.clone())),
        )?;
        let extent = [(960. * dpr) as u32, (540. * dpr) as u32];
        let mut strip = image::RgbaImage::new(extent[0] * 3, extent[1] * 2);
        for (index, stage) in ["empty", "edit", "reset", "full", "saved", "restored"]
            .into_iter()
            .enumerate()
        {
            match stage {
                "edit" => {
                    for palette in [0, 0, 37, 41, 41, 39] {
                        login.studio.state.edit_builder(|b| {
                            b.append(palette);
                        });
                    }
                }
                "reset" => login.studio.activate(RESET, "Builder"),
                "full" => {
                    for n in 0..47 {
                        login.studio.state.edit_builder(|b| {
                            b.append(PALETTES[n % 39]);
                        });
                    }
                }
                "saved" => {
                    let mut bytes = [19, 237].into_iter();
                    login.studio.activate_with_random(SAVE, "Builder", &mut || {
                        bytes.next().expect("two header samples")
                    });
                    login.step(10304., false, false)?;
                }
                "restored" => {
                    let mut storage = MenuStorage::at(Some(directory.clone()));
                    let restored = storage.restore();
                    assert_eq!(&restored.record, login.studio.state.committed());
                    assert_eq!(
                        &restored.record.custom_bytes[..8],
                        &[255, 255, 255, 0, 0, 0, 19, 237]
                    );
                    renderer.sync_resources(&[])?;
                    login = opened(restored.record, storage)?;
                }
                _ => {}
            }
            if !["saved", "restored"].contains(&stage) {
                login.step(10010. + index as f64 * 16., false, false)?;
            }
            let path = output.join(format!("builder-{stage}-{dpr}x.png"));
            shot(renderer, &mut login, [960., 540.], dpr, &path)?;
            let image = image::open(&path).map_err(|e| e.to_string())?.to_rgba8();
            image::imageops::replace(
                &mut strip,
                &image,
                (index % 3) as i64 * i64::from(extent[0]),
                (index / 3) as i64 * i64::from(extent[1]),
            );
            login.studio.presented();
        }
        strip
            .save(output.join(format!("edit-save-restore-strip-{dpr}x.png")))
            .map_err(|e| e.to_string())?;
        for (name, logical) in [("narrow", [360., 640.]), ("short-wide", [960., 360.])] {
            for bottom in [false, true] {
                if bottom {
                    login.studio.state.reveal_control(RESET, logical);
                } else {
                    login.studio.state.reveal_control(
                        slither_client::menu_controls::ControlId(SWATCH_BASE),
                        logical,
                    );
                }
                shot(
                    renderer,
                    &mut login,
                    logical,
                    dpr,
                    &output.join(format!(
                        "builder-{name}-{}-{dpr}x.png",
                        if bottom { "actions" } else { "swatches" }
                    )),
                )?;
            }
        }
    }
    // Numeric resource envelope includes all 39 single-colour previews, the full pattern,
    // and large/opaque retained patterns. No official art or network adapter is used.
    let mut ledger = vec![
        "dpr,case,logical_width,logical_height,egui_shapes,preview_draws,preview_slots,preview_bytes"
            .to_owned(),
    ];
    for dpr in [1., 1.5, 2., 3., 4., 5.] {
        let patterns: Vec<_> = PALETTES
            .iter()
            .map(|p| (format!("palette-{p}"), vec![*p]))
            .chain([
                (
                    "full-47".into(),
                    (0..47).map(|n| PALETTES[n % 39]).collect(),
                ),
                ("rle-47".into(), vec![41; 47]),
            ])
            .collect();
        for (name, colours) in patterns {
            let mut login = opened(AppearanceRecord::default(), MenuStorage::at(None))?;
            for colour in colours {
                login.studio.state.edit_builder(|b| {
                    b.append(colour);
                });
            }
            record_ledger(&mut login, dpr, &name, &mut ledger)?;
        }
        for (name, bytes) in [
            ("legacy-large", [vec![0; 8], [255, 41].repeat(123)].concat()),
            ("opaque-prefix", vec![1, 2, 3, 4, 5, 6, 7, 8, 2, 41]),
            ("raw-255", vec![9; 255]),
        ] {
            let mut login = opened(
                AppearanceRecord {
                    builtin: 65,
                    custom_base: 65,
                    custom_bytes: bytes,
                    active_custom: true,
                    ..Default::default()
                },
                MenuStorage::at(None),
            )?;
            record_ledger(&mut login, dpr, name, &mut ledger)?;
            if dpr <= 2. && name == "legacy-large" {
                renderer.sync_resources(&[])?;
                shot(
                    renderer,
                    &mut login,
                    [960., 540.],
                    dpr,
                    &output.join(format!("builder-legacy-large-{dpr}x.png")),
                )?;
            }
        }
    }
    std::fs::write(output.join("dpr-ledger.csv"), ledger.join("\n") + "\n")
        .map_err(|e| e.to_string())?;
    std::fs::write(output.join("README.txt"),"Authored offline builder specimens. No windows, sockets, official code or art.\nStrip rows: empty, edit, Reset / 47 colours, saved login, restored custom.\nClocks: initial 10010ms, edits at 10026/10042/10058, Save at 10058 and returned at 10304; restore repeats the same opening clock. Header tape [19,237] sampled only at changed Save.\nSame logical 960x540 at DPR 1/1.5/2; narrow and short-wide scrolling states included. Numeric ledger spans DPR 1-5 including 1.5.\nOffscreen synthetic layout/resource evidence only. Physical input, OS accessibility, owner feel and server acceptance unverified.\n").map_err(|e|e.to_string())?;
    Ok(())
}
