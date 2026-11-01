//! Headless CPU planning/allocation workload; timing requires --measure.
#[path = "../tests/support/capacity_world.rs"]
mod synthetic;
#[path = "../tests/support/announcement_tape.rs"]
mod tape;
use slither_wgpu_slice::{Result, render};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    time::Instant,
};

struct Counting;
thread_local! { static BYTES: Cell<Option<u64>> = const { Cell::new(None) }; }
// Observe requested Rust bytes on this thread; forward every allocator contract
// unchanged. GPU/driver memory, resident heap and worker threads are excluded.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        BYTES.with(|b| b.set(b.get().map(|n| n + layout.size() as u64)));
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        BYTES.with(|b| b.set(b.get().map(|n| n + layout.size() as u64)));
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        BYTES.with(|b| b.set(b.get().map(|n| n + size as u64)));
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOC: Counting = Counting;

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let measure = args == ["--measure"];
    if !measure && args != ["--functional"] && args != ["--functional-256"] {
        return Err("Expected --functional or --measure (book a quiet window first)".into());
    }
    println!(
        "round,population,anchors,frame,cpu_ns,allocated_bytes,draws,visible_snakes,pixel_bytes,body_payload_bytes"
    );
    for points in if measure { vec![26, 4096] } else { vec![26] } {
        let populations = if args == ["--functional-256"] {
            vec![256]
        } else {
            vec![256, 512]
        };
        let mut games: Vec<_> = populations
            .iter()
            .map(|&n| synthetic::world(n, points))
            .collect::<Result<_>>()?;
        let mut worlds: Vec<_> = games
            .iter()
            .map(|g| {
                let mut c = g.client().clone();
                c.wheel_zoom(-1000.);
                c
            })
            .collect();
        for world in &mut worlds {
            world.advance(1003., false).map_err(|e| e.to_string())?;
            for _ in 0..110 {
                // Settle the production camera without spending the quiet window
                // on full-detail warmup pictures. Normal quality is restored for
                // every measured picture and its preceding warm frames.
                world
                    .prepare_draw_with_style(synthetic::EXTENT[0], synthetic::EXTENT[1], 1e6, false)
                    .map_err(|e| e.to_string())?;
            }
        }
        for round in 0..if measure { 5 } else { 1 } {
            for index in if round % 2 == 0 { [0, 1] } else { [1, 0] } {
                if index >= games.len() {
                    continue;
                }
                let game = &mut games[index];
                let world = &mut worlds[index];
                let quality = game.quality();
                let hud = game.hud_view();
                let warm = if points == 4096 { 5 } else { 10 };
                let count = if points == 4096 { 20 } else { 40 };
                for frame in 0..if measure { count } else { 1 } {
                    BYTES.with(|b| b.set(Some(0)));
                    let start = measure.then(Instant::now);
                    let p = render::prepare_gameplay(
                        world,
                        &mut game.bank,
                        synthetic::EXTENT,
                        1.,
                        1000 + frame,
                        quality,
                    )?;
                    let p = render::finish_hud(p, &mut game.bank, &hud)?;
                    let ns = start.map_or(0, |s| s.elapsed().as_nanos());
                    let bytes = BYTES.with(|b| b.replace(None).unwrap());
                    assert_eq!(p.frame.snakes.len(), [256, 512][index]);
                    assert!(p.frame.snakes.iter().all(|s| s.body_commands > 0));
                    assert!(p.draw_count() <= slither_wgpu_slice::frame::MAX_DRAWS);
                    assert!(p.pixel_bytes() <= slither_wgpu_slice::bank::PIXEL_BUDGET);
                    if !measure || frame >= warm {
                        println!(
                            "{round},{},{points},{frame},{ns},{bytes},{},{},{},{}",
                            [256, 512][index],
                            p.draw_count(),
                            p.frame.snakes.len(),
                            p.pixel_bytes(),
                            world.retained_body_points()
                                * size_of::<slither_client::presentation::BodyPoint>()
                        );
                    }
                    if !measure && let Some(path) = std::env::var_os("SNAKE_CAPACITY_SNAPSHOT") {
                        std::fs::write(path, format!("{:?}", p.frame))
                            .map_err(|e| e.to_string())?;
                    }
                    game.bank.recycle_frame(p);
                }
            }
        }
    }
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
