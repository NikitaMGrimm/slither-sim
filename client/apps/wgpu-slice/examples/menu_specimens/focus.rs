//! Fixed-clock native focus and transition specimens, using only procedural fixtures.
use super::*;
use slither_client::{
    login_controls as login_ids, menu_controls::ControlId, settings, skin_studio::*,
};
use slither_wgpu_slice::{login_native::NativeLogin, menu_storage::MenuStorage};

fn fixture_art() -> Result<Cosmetics> {
    let mut bytes = Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(32, 24, image::Rgba([100, 160, 220, 220]))
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    let mut images = Images::default();
    for (name, ..) in slither_wgpu_slice::cosmetics::ACCESSORIES {
        images.add(&format!("s/a_{name}.png"), bytes.get_ref())?;
    }
    Cosmetics::from_images(images, None)
}

struct Sweep<'a> {
    renderer: &'a mut Renderer,
    output: &'a Path,
    ledger: Vec<serde_json::Value>,
    logical: [f64; 2],
    dpr: f64,
    ready: bool,
}
impl Sweep<'_> {
    fn sample(
        &mut self,
        login: &mut NativeLogin,
        art: &mut Cosmetics,
        name: &str,
        focus: Option<ControlId>,
        hover: bool,
    ) -> Result<()> {
        let extent = self.logical.map(|v| (v * self.dpr).round() as u32);
        if let Some(id) = focus {
            login.studio.state.reveal_control(id, self.logical);
        }
        login.studio.sync_art(self.logical, Some(art))?;
        login
            .prepare_host(extent, self.dpr, true, Some(art))
            .map_err(|e| {
                format!(
                    "{name} {:?} DPR{} ready{} initial: {e}",
                    self.logical, self.dpr, self.ready
                )
            })?;
        if let Some(id) = focus {
            login.controls.keyboard_navigation();
            login.focus(id);
        }
        for control in &login.panel.controls {
            if control.enabled {
                login
                    .controls
                    .set_progress(control.id, if hover { [1., 1.] } else { [0.; 2] });
            }
        }
        let mut menu = login
            .prepare_host(extent, self.dpr, true, Some(art))
            .map_err(|e| {
                format!(
                    "{name} {:?} DPR{} ready{} focused: {e}",
                    self.logical, self.dpr, self.ready
                )
            })?;
        if let Some(id) = focus
            && login.controls.focus() != Some(id)
        {
            return Err(format!("Focus fixture lost {id:?} in {name}"));
        }
        let studio = login.studio.state.visible();
        let counts = menu.draw_counts(0);
        let records = counts.admitted_menu();
        let bytes: usize = menu.resources.iter().map(|r| r.image.pixels.len()).sum();
        if records > if studio { 448 } else { 512 }
            || menu.resources.len() > 64
            || bytes > 13 * 1024 * 1024
        {
            return Err(format!(
                "Focus allowance {name} {:?} DPR{}: {records} records, {} slots, {bytes} bytes",
                self.logical,
                self.dpr,
                menu.resources.len()
            ));
        }
        if counts.components.total() != menu.frame.draws.len() {
            return Err("Focus attribution mismatch".into());
        }
        self.ledger.push(serde_json::json!({"case":name,"logical":self.logical,"dpr":self.dpr,"ready_art":self.ready,"studio":studio,"records":records,"slots":menu.resources.len(),"bytes":bytes,"focus":format!("{:?}",login.controls.focus())}));
        let wide = self.logical == [960., 540.];
        let screenshot = !self.ready
            && [1., 1.5, 2.].contains(&self.dpr)
            && (focus.is_some() || name.starts_with("roundtrip-"))
            && (wide || name == "cosmetics-cell" || name == "settings-toggle");
        if screenshot {
            if studio {
                let (preview, resources) =
                    login.studio.prepare_preview(extent, self.dpr, Some(art))?;
                menu.frame.draws.extend(preview.draws);
                menu.resources.extend(resources);
            }
            if let Some(cover) = menu.world_cover.take() {
                menu.frame.draws.insert(0, cover);
            }
            capture(
                self.renderer,
                &menu,
                &self.output.join(format!(
                    "{name}-{}x{}-{}x.png",
                    self.logical[0], self.logical[1], self.dpr
                )),
            )?;
            login.studio.presented();
        }
        Ok(())
    }
    fn run(&mut self) -> Result<()> {
        let mut art = if self.ready {
            fixture_art()?
        } else {
            Cosmetics::from_images(Images::default(), None)?
        };
        let mut login = NativeLogin::with_preferences(
            AppearanceRecord {
                builtin: 7,
                nickname: "Focus".into(),
                ..Default::default()
            },
            MenuStorage::at(None),
        )?;
        for now in (0..=9600).step_by(16) {
            login.step(f64::from(now), true, false)?;
        }
        for (name, id) in [
            ("login-field", login_ids::NICKNAME),
            ("login-play", login_ids::PLAY),
            ("login-settings", login_ids::SETTINGS),
            ("login-skin", login_ids::CHANGE_SKIN),
        ] {
            self.sample(&mut login, &mut art, name, Some(id), false)?;
        }
        login.open_settings(true);
        for (name, id) in [
            ("settings-toggle", settings::QUALITY),
            ("settings-resolution", settings::RESOLUTION),
            ("settings-done", settings::DONE),
        ] {
            self.sample(&mut login, &mut art, name, Some(id), false)?;
        }
        login.close_settings(true);
        login.step(10000., true, false)?;
        login.open_studio();
        for t in [0., 1., 90., 115., 180., 229., 230.] {
            login.step(10000. + t, true, false)?;
            self.sample(
                &mut login,
                &mut art,
                &format!("builtin-entry-{t}"),
                None,
                t == 1.,
            )?;
        }
        for (name, id) in [
            ("builtin-tab", BUILT_IN_TAB),
            ("builtin-previous", PREVIOUS),
            ("builtin-save", SAVE),
        ] {
            self.sample(&mut login, &mut art, name, Some(id), false)?;
        }
        login.studio.activate(BUILDER_TAB, "Focus");
        login.studio.state.edit_builder(|builder| {
            builder.append(1);
            builder.append(2);
        });
        for t in [0., 90., 180.] {
            login.step(10230. + t, true, false)?;
            self.sample(
                &mut login,
                &mut art,
                &format!("builder-fade-{t}"),
                None,
                t == 90.,
            )?;
        }
        for (name, id) in [
            ("builder-tab", BUILDER_TAB),
            ("builder-swatch", ControlId(SWATCH_BASE + 2)),
            ("builder-run", ControlId(RUN_BASE)),
            ("builder-add", ADD),
        ] {
            self.sample(&mut login, &mut art, name, Some(id), false)?;
        }
        login.studio.activate(COSMETICS_TAB, "Focus");
        for t in [0., 90., 180.] {
            login.step(10410. + t, true, false)?;
            self.sample(
                &mut login,
                &mut art,
                &format!("cosmetics-fade-{t}"),
                None,
                t == 90.,
            )?;
        }
        login.studio.state.select_cosmetic(Some(0));
        self.sample(
            &mut login,
            &mut art,
            "cosmetics-cell",
            Some(ControlId(COSMETIC_BASE + 1)),
            false,
        )?;
        self.sample(
            &mut login,
            &mut art,
            "cosmetics-next",
            Some(COSMETIC_NEXT),
            false,
        )?;
        login.studio.state.select_cosmetic(Some(31));
        self.sample(
            &mut login,
            &mut art,
            "cosmetics-last",
            Some(ControlId(COSMETIC_BASE + 32)),
            true,
        )?;
        login.studio.activate(CANCEL, "Focus");
        for t in [0., 90., 230.] {
            login.step(10590. + t, true, false)?;
            self.sample(
                &mut login,
                &mut art,
                &format!("studio-return-{t}"),
                None,
                false,
            )?;
        }
        login.open_studio();
        login.studio.activate(BUILDER_TAB, "Focus");
        for t in [0., 1., 90., 115., 180., 229., 230.] {
            login.step(10820. + t, true, false)?;
            self.sample(
                &mut login,
                &mut art,
                &format!("early-builder-{t}"),
                None,
                t == 1.,
            )?;
        }
        for keyboard in [false, true] {
            self.renderer.sync_resources(&[])?;
            let mut roundtrip = NativeLogin::new("Focus")?;
            for now in (0..=9600).step_by(16) {
                roundtrip.step(now as f64, true, false)?;
            }
            roundtrip.prepare_host(
                self.logical.map(|v| (v * self.dpr).round() as u32),
                self.dpr,
                true,
                None,
            )?;
            let logical = self.logical;
            let click = |login: &mut NativeLogin, id| -> Result<()> {
                let resolved = login.panel.resolve(logical).map_err(str::to_owned)?;
                let r = resolved
                    .iter()
                    .find(|r| r.control.id == id)
                    .ok_or("Missing click fixture")?;
                let point = [r.rect.x + r.rect.width / 2., r.rect.y + r.rect.height / 2.];
                login.pointer(point, Some(true), true);
                login.pointer(point, Some(false), true);
                Ok(())
            };
            if keyboard {
                roundtrip.controls.keyboard_navigation();
                roundtrip.focus(login_ids::CHANGE_SKIN);
                roundtrip.activate(true);
            } else {
                click(&mut roundtrip, login_ids::CHANGE_SKIN)?;
            }
            roundtrip.step(9900., true, false)?;
            let modality = if keyboard { "keyboard" } else { "mouse" };
            self.sample(
                &mut roundtrip,
                &mut art,
                &format!("roundtrip-{modality}-open"),
                None,
                false,
            )?;
            if keyboard {
                roundtrip.focus(CANCEL);
                roundtrip.activate(true);
            } else {
                click(&mut roundtrip, CANCEL)?;
            }
            roundtrip.step(10200., true, false)?;
            roundtrip.sync_return_focus(true);
            self.sample(
                &mut roundtrip,
                &mut art,
                &format!("roundtrip-{modality}-return"),
                None,
                false,
            )?;
        }
        Ok(())
    }
}

pub async fn specimens(renderer: &mut Renderer, output: &Path) -> Result<()> {
    let validation = renderer
        .device
        .push_error_scope(wgpu::ErrorFilter::Validation);
    let mut sweep = Sweep {
        renderer,
        output,
        ledger: Vec::new(),
        logical: [960., 540.],
        dpr: 1.,
        ready: false,
    };
    for dpr in [1., 1.25, 1.5, 1.75, 2., 3., 4., 5., 6., 7., 8.] {
        for logical in [[960., 540.], [360., 640.], [960., 360.]] {
            for ready in [false, true] {
                sweep.renderer.sync_resources(&[])?;
                sweep.logical = logical;
                sweep.dpr = dpr;
                sweep.ready = ready;
                sweep.run()?;
            }
        }
    }
    if let Some(error) = validation.pop().await {
        return Err(error.to_string());
    }
    let revision = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| e.to_string())?;
    let result = serde_json::json!({"revision":String::from_utf8_lossy(&revision.stdout).trim(),"fixture":"fixed-clock synthetic focus; procedural solid art for budgets; screenshots use placeholders only; no art folders or servers","cases":sweep.ledger});
    std::fs::write(
        output.join("focus-ledger.json"),
        serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
