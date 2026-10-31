//! Finite CPU-only wordmark workload. No window, GPU, browser or game socket.
use slither_client::menu_layout::MenuRect;
use slither_wgpu_slice::{Result, menu::Menu, menu_specimen, text::TextCache, wordmark};
use std::{hint::black_box, io::Write, path::Path, time::Instant};

fn run(output: &Path, count: usize, progress: f64) -> Result<()> {
    std::fs::create_dir(output).map_err(|e| e.to_string())?;
    let mut csv = std::fs::File::create(output.join("times.csv")).map_err(|e| e.to_string())?;
    writeln!(csv, "phase,dpr,iteration,ms").map_err(|e| e.to_string())?;
    let layout = wordmark::layout(
        wordmark::DEFAULT_TEXT,
        MenuRect {
            x: 735.,
            y: 50.,
            width: 450.,
            height: 115.,
        },
        1920.,
        |_| Err("unexpected fallback".into()),
    )?;
    for dpr in [1., 2., 5.] {
        let mut last = None;
        for i in 0..count + 4 {
            let now = Instant::now();
            let image = wordmark::raster(black_box(&layout), dpr, black_box(progress))?;
            let elapsed = now.elapsed().as_secs_f64() * 1000.;
            if !image.pixels.iter().any(|v| *v > 0) {
                return Err("empty timed raster".into());
            }
            black_box(&image);
            if i >= 4 {
                writeln!(csv, "raster,{dpr},{},{elapsed:.6}", i - 4).map_err(|e| e.to_string())?;
            }
            last = Some(image);
        }
        std::fs::write(
            output.join(format!("reveal-{dpr}.rgba")),
            last.unwrap().pixels,
        )
        .map_err(|e| e.to_string())?;
        let settled = wordmark::raster(&layout, dpr, 1.)?;
        std::fs::write(output.join(format!("settled-{dpr}.rgba")), settled.pixels)
            .map_err(|e| e.to_string())?;

        let mut text = TextCache::menu();
        let mut menu = Menu::default();
        let mut view = menu_specimen::view(0)?;
        let extent = [(960. * dpr) as u32, (540. * dpr) as u32];
        let geometry = menu_specimen::layout(&mut text, &view, extent, dpr)?;
        for i in 0..count + 4 {
            // A 40 ms time advance forces a new reveal raster on both the old
            // cadence and the 25 ms gate. This times the complete CPU menu path.
            view.logo_reveal = 0.2 + i as f64 * 0.006;
            let now = Instant::now();
            let picture = menu.prepare(&mut text, &view, &geometry, extent, dpr, None)?;
            let elapsed = now.elapsed().as_secs_f64() * 1000.;
            if picture.frame.draws.is_empty() {
                return Err("empty timed menu".into());
            }
            black_box(&picture);
            if i >= 4 {
                writeln!(csv, "menu,{dpr},{},{elapsed:.6}", i - 4).map_err(|e| e.to_string())?;
            }
        }
    }
    std::fs::write(output.join("workload.txt"), format!(
        "release CPU raster at progress {progress}; {count} samples after 4 warmups per DPR; raw RGBA dimensions from raster_size; complete menu sweep at 960x540 CSS, 40 ms advances; no GPU or network\n"
    )).map_err(|e| e.to_string())?;
    Ok(())
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let result = (|| {
        let output = args.get(1).ok_or("new output directory required")?;
        let count = args
            .get(2)
            .map(|s| s.parse::<usize>())
            .transpose()
            .map_err(|e| e.to_string())?
            .unwrap_or(60);
        let progress = args
            .get(3)
            .map(|s| s.parse::<f64>())
            .transpose()
            .map_err(|e| e.to_string())?
            .unwrap_or(0.5);
        if !(16..=128).contains(&count)
            || !progress.is_finite()
            || !(0. ..1.).contains(&progress)
            || progress == 0.
        {
            return Err(
                "samples must be 16..128 and reveal progress strictly between zero and one".into(),
            );
        }
        run(Path::new(output), count, progress)
    })();
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
