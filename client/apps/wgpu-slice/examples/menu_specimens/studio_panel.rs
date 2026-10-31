//! Fixed-clock studio comparison mode; shares the menu exporter and GPU readback.
use super::{Renderer, readback};
use image::{
    ImageEncoder,
    codecs::png::{CompressionType, FilterType, PngEncoder},
};
use slither_client::{
    menu_controls::{ControlKind, PanelInputState},
    menu_input::MenuInputState,
    menu_layout::MenuRect,
    skin_studio::*,
};
use slither_wgpu_slice::{
    Result,
    login_native::NativeLogin,
    menu::{Menu, MenuFrame},
    menu_counts::MenuComponent,
    text::TextCache,
};
use std::{collections::BTreeMap, fs, path::Path};

#[allow(clippy::too_many_arguments)]
fn capture(
    renderer: &mut Renderer,
    menu: &MenuFrame,
    preview: usize,
    name: &str,
    out: &Path,
    reference: Option<&Path>,
    ledger: &mut String,
    differences: &mut String,
) -> Result<bool> {
    let counts = menu.draw_counts(preview);
    if counts.components.total() + preview != menu.frame.draws.len() {
        return Err(format!("Attribution mismatch: {name}"));
    }
    if counts.admitted_menu() > 512 {
        return Err(format!("Menu quota: {name}"));
    }
    let components = MenuComponent::ALL
        .map(|c| counts.components.get(c).to_string())
        .join(",");
    ledger.push_str(&format!(
        "{name},{components},{},{},{},{},{},{}\n",
        counts.page,
        counts.cover,
        preview,
        counts.reported_menu(),
        counts.admitted_menu(),
        counts.combined()
    ));
    let mut frame = menu.frame.clone();
    if let Some(cover) = &menu.world_cover {
        frame.draws.insert(0, cover.clone());
    }
    if let Some(page) = &menu.backdrop {
        frame.draws.insert(0, page.clone());
    }
    let rgba = readback(renderer, &frame, &menu.resources, None)?;
    let mut mismatch = false;
    let [w, h] = menu.frame.extent;
    if let Some(reference) = reference {
        let before = image::open(reference.join(format!("{name}.png")))
            .map_err(|e| e.to_string())?
            .to_rgba8();
        if before.dimensions() != (w, h) {
            return Err("Comparison extent mismatch".into());
        }
        let (delta, at, changed) = difference(before.as_raw(), &rgba);
        mismatch = delta != 0;
        differences.push_str(&format!(
            "{name},{delta},{},{},{},{changed}\n",
            at / 4 % w as usize,
            at / 4 / w as usize,
            at % 4
        ));
    }
    PngEncoder::new_with_quality(
        fs::File::create(out.join(format!("{name}.png"))).map_err(|e| e.to_string())?,
        CompressionType::Fast,
        FilterType::NoFilter,
    )
    .write_image(&rgba, w, h, image::ExtendedColorType::Rgba8)
    .map_err(|e| e.to_string())?;
    Ok(mismatch)
}

fn difference(before: &[u8], after: &[u8]) -> (u8, usize, u64) {
    let (mut delta, mut at, mut changed) = (0, 0, 0);
    if before != after {
        for (i, (a, b)) in before.iter().zip(after).enumerate() {
            let d = a.abs_diff(*b);
            if d > delta {
                delta = d;
                at = i;
            }
            changed += u64::from(d != 0);
        }
    }
    (delta, at, changed)
}
fn finish(out: &Path, ledger: &str, differences: &str, mismatches: usize) -> Result<()> {
    fs::write(out.join("draws.csv"), ledger).map_err(|e| e.to_string())?;
    fs::write(out.join("pixels.csv"), differences).map_err(|e| e.to_string())?;
    if mismatches != 0 {
        return Err(format!(
            "{mismatches} studio pixel comparisons differ; evidence saved in {}",
            out.display()
        ));
    }
    Ok(())
}

pub async fn specimens(
    renderer: &mut Renderer,
    out: &Path,
    reference: Option<&Path>,
) -> Result<()> {
    let validation = renderer
        .device
        .push_error_scope(wgpu::ErrorFilter::Validation);
    let mut mismatches = 0;
    let mut ledger = String::from(
        "case,login_wordmark,login_controls,login_text,studio_shade,studio_surface,studio_chrome,studio_section,other,page,cover,preview,reported_menu,admitted_menu,combined\n",
    );
    let mut differences = String::from("case,max_channel_delta,x,y,channel,changed_channels\n");
    for dpr in [1., 1.25, 1.5, 2., 3., 4., 5., 6., 7., 8.] {
        for (layout, logical) in [
            ("wide", [960., 540.]),
            ("narrow", [360., 640.]),
            ("short", [960., 360.]),
        ] {
            let extent = logical.map(|n| (n * dpr) as u32);
            for builder in [false, true] {
                renderer.sync_resources(&[])?;
                let mut login = NativeLogin::new("Player")?;
                for now in (0..=9600).step_by(16) {
                    login.step(now as f64, false, false)?;
                }
                login.prepare_host(extent, dpr, false, None)?;
                login.open_studio();
                if builder {
                    // Deliberately switch before entry finishes: the former 494 case.
                    login.studio.activate(BUILDER_TAB, "Player");
                    for n in 0..47 {
                        login.studio.state.edit_builder(|b| {
                            b.append(slither_client::skin_builder::PALETTES[n % 39]);
                        });
                    }
                }
                for time in [0., 1., 90., 115., 180., 229., 230., 410., 525., 639., 640.] {
                    login.step(9600. + time, false, false)?;
                    if time == 410. {
                        login.studio.state.cancel(10010.);
                        login.step(10010., false, false)?;
                    }
                    let mut menu = login.prepare_host(extent, dpr, false, None)?;
                    let preview = if login.studio.state.visible() {
                        let (frame, resources) = login.studio.prepare_preview(extent, dpr, None)?;
                        let count = frame.draws.len();
                        menu.frame.draws.extend(frame.draws);
                        menu.resources.extend(resources);
                        count
                    } else {
                        0
                    };
                    let name = format!(
                        "{layout}-{}-{dpr}x-{time:03}",
                        if builder { "builder" } else { "builtin" }
                    );
                    mismatches += usize::from(capture(
                        renderer,
                        &menu,
                        preview,
                        &name,
                        out,
                        reference,
                        &mut ledger,
                        &mut differences,
                    )?);
                    login.studio.presented();
                }
                // Explicit fractional clip cuts the surface/shadow; other controls stay intact.
                renderer.sync_resources(&[])?;
                let mut state = StudioState::new(AppearanceRecord {
                    builtin: 7,
                    ..Default::default()
                })
                .map_err(str::to_owned)?;
                state.open(0., true);
                state.step(230.);
                if builder {
                    state.set_section(StudioSection::Builder, 230.);
                    state.step(410.);
                }
                let mut panel = state.panel(logical, "");
                let bounds = StudioState::bounds(logical);
                panel
                    .controls
                    .iter_mut()
                    .find(|c| matches!(c.kind, ControlKind::Surface))
                    .unwrap()
                    .clip = Some(MenuRect {
                    x: bounds.x + 3.25,
                    y: bounds.y + 5.5,
                    width: bounds.width - 19.75,
                    height: bounds.height - 11.25,
                });
                let menu = Menu::default().prepare_panel(
                    &mut TextCache::menu(),
                    &panel,
                    &PanelInputState::default(),
                    &MenuInputState::default(),
                    &mut BTreeMap::new(),
                    Default::default(),
                    extent,
                    dpr,
                )?;
                mismatches += usize::from(capture(
                    renderer,
                    &menu,
                    0,
                    &format!(
                        "{layout}-{}-{dpr}x-clipped",
                        if builder { "builder" } else { "builtin" }
                    ),
                    out,
                    reference,
                    &mut ledger,
                    &mut differences,
                )?);
            }
        }
        fs::write(out.join("draws.csv"), &ledger).map_err(|e| e.to_string())?;
        fs::write(out.join("pixels.csv"), &differences).map_err(|e| e.to_string())?;
        println!("Completed DPR {dpr}");
    }
    let result = finish(out, &ledger, &differences, mismatches);
    if let Some(error) = validation.pop().await {
        return Err(error.to_string());
    }
    println!("Completed {} cases", ledger.lines().count() - 1);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_mismatches_fail_only_after_saving_the_complete_ledgers() {
        let before = [0, 20, 30, 255, 40, 50, 60, 255];
        let mut after = before;
        after[0] = 1;
        after[6] = 57;
        assert_eq!(difference(&before, &before), (0, 0, 0));
        assert_eq!(difference(&before, &after), (3, 6, 2));
        let out = std::env::temp_dir().join(format!(
            "studio-comparison-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&out).unwrap();
        let ledger = "case,reported_menu\nchanged,1\nmatching,2\nchanged_again,3\n";
        let differences = "case,max_channel_delta\nchanged,3\nmatching,0\nchanged_again,1\n";
        assert!(
            finish(&out, ledger, differences, 2)
                .unwrap_err()
                .starts_with("2 studio")
        );
        assert_eq!(fs::read_to_string(out.join("draws.csv")).unwrap(), ledger);
        assert_eq!(
            fs::read_to_string(out.join("pixels.csv")).unwrap(),
            differences
        );
        assert!(finish(&out, ledger, differences, 0).is_ok());
        fs::remove_file(out.join("draws.csv")).unwrap();
        fs::remove_file(out.join("pixels.csv")).unwrap();
        fs::remove_dir(out).unwrap();
    }
}
