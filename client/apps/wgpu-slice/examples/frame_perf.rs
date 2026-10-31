//! Bounded synthetic native frame workload: no windows, sockets or external assets.
#[cfg(windows)]
use slither_wgpu_slice::{Result, gpu, login_native::NativeLogin, render};
#[cfg(windows)]
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    path::Path,
    sync::mpsc,
    time::{Duration, Instant},
};
#[cfg(windows)]
#[path = "frame_perf/food.rs"]
mod food;
#[cfg(windows)]
#[path = "../tests/support/announcement_tape.rs"]
mod tape;
#[cfg(windows)]
#[path = "frame_perf/zoom.rs"]
mod zoom;

#[cfg(windows)]
struct Counting;
#[cfg(windows)]
thread_local! {
    static COUNT: Cell<Option<u64>> = const { Cell::new(None) };
}
// Delegate unchanged layouts/pointers to System; count requested bytes only on
// the measuring thread, excluding driver heaps and asynchronous GPU workers.
#[cfg(windows)]
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        COUNT.with(|c| c.set(c.get().map(|n| n + layout.size() as u64)));
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        COUNT.with(|c| c.set(c.get().map(|n| n + layout.size() as u64)));
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        COUNT.with(|c| c.set(c.get().map(|n| n + size as u64)));
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[cfg(windows)]
#[global_allocator]
static ALLOC: Counting = Counting;

#[cfg(windows)]
fn measure<T>(f: impl FnOnce() -> Result<T>) -> Result<(T, u128, u64)> {
    COUNT.with(|c| c.set(Some(0)));
    let start = Instant::now();
    let result = f();
    let ns = start.elapsed().as_nanos();
    let bytes = COUNT.with(|c| c.replace(None).unwrap());
    result.map(|v| (v, ns, bytes))
}
#[cfg(windows)]
fn wait(r: &gpu::Renderer) -> Result<()> {
    r.device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(10)),
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}
#[cfg(windows)]
fn target(r: &gpu::Renderer) -> wgpu::Texture {
    r.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("frame-perf target"),
        size: wgpu::Extent3d {
            width: 960,
            height: 540,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}
#[cfg(windows)]
fn capture(r: &gpu::Renderer, texture: &wgpu::Texture, path: &Path) -> Result<()> {
    let buffer = r.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("frame-perf readback"),
        size: 960 * 540 * 4,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = r.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(3840),
                rows_per_image: Some(540),
            },
        },
        texture.size(),
    );
    r.queue.submit([encoder.finish()]);
    let (tx, rx) = mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |v| {
        let _ = tx.send(v);
    });
    wait(r)?;
    rx.recv_timeout(Duration::from_secs(1))
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    {
        let data = buffer
            .slice(..)
            .get_mapped_range()
            .map_err(|e| e.to_string())?;
        std::fs::write(path, &data).map_err(|e| e.to_string())?;
    }
    buffer.unmap();
    Ok(())
}
#[cfg(windows)]
fn world() -> Result<slither_wgpu_slice::game::LocalGame> {
    let mut g = tape::playing();
    let mut packets = Vec::new();
    // 24 long snakes and 2400 food in a viewport-sized corridor. This is a
    // density stress fixture, not an empirical claim about official populations.
    for i in 0..24_u16 {
        let x = 900 + (i % 6) * 65;
        let y = 800 + (i / 6) * 110;
        let mut p = vec![b's'];
        p.extend((100 + i).to_be_bytes());
        p.extend(tape::u24(0));
        p.push(b'0');
        p.extend(tape::u24(0));
        p.extend(4250_u16.to_be_bytes());
        p.extend(tape::u24(0));
        p.push(0);
        p.extend(tape::u24(u32::from(x) * 5));
        p.extend(tape::u24(u32::from(y) * 5));
        p.extend([0, 0, 255]);
        p.extend(tape::u24(u32::from(x) * 5 - 1600));
        p.extend(tape::u24(u32::from(y) * 5));
        for _ in 2..120 {
            p.extend([132, 127]);
        }
        p.extend([0, 0]);
        packets.push(p);
    }
    for i in 0..2400_u16 {
        let x = 700 + (i % 60) * 12;
        let y = 700 + (i / 60) * 15;
        let p = vec![
            b'F',
            (x / 100) as u8,
            (y / 100) as u8,
            0,
            ((x % 100) * 255 / 100) as u8,
            ((y % 100) * 255 / 100) as u8,
            20,
        ];
        packets.push(p);
    }
    g.receive(g.generation(), &tape::bundle(&packets), 21.)?;
    tape::pictures(&mut g);
    tape::pump(&mut g, 1000.);
    g.window_viewport([960, 540], 1.)?;
    Ok(g)
}
#[cfg(windows)]
fn run(output: &Path) -> Result<()> {
    std::fs::create_dir(output).map_err(|e| e.to_string())?;
    let instance = gpu::instance();
    let mut seen = std::collections::BTreeSet::new();
    let backends = if std::env::var_os("FRAME_PERF_VULKAN").is_some() {
        wgpu::Backends::VULKAN
    } else {
        wgpu::Backends::DX12
    };
    for adapter in pollster::block_on(instance.enumerate_adapters(backends)) {
        let info = adapter.get_info();
        if info.device_type == wgpu::DeviceType::Cpu {
            continue;
        }
        if !seen.insert((info.vendor, info.device)) {
            continue;
        }
        let name = format!("{:04x}-{:04x}", info.vendor, info.device);
        let dir = output.join(&name);
        std::fs::create_dir(&dir).map_err(|e| e.to_string())?;
        println!("adapter={info:?}");
        let mut r = pollster::block_on(gpu::Renderer::from_adapter(
            adapter,
            wgpu::TextureFormat::Rgba8Unorm,
        ))?;
        let scope = r.device.push_error_scope(wgpu::ErrorFilter::Validation);
        println!("samples={}", r.sample_count);
        let texture = target(&r);
        let view = texture.create_view(&Default::default());
        let mut csv = String::from(
            "scene,frame,draws,pixels,prepare_ns,sync_ns,submit_ns,prepare_bytes,sync_bytes,submit_bytes\n",
        );
        for mode in ["world", "hud", "login"] {
            let mut game = world()?;
            let mut world = game.client().clone();
            let mut hud = game.hud_view();
            hud.score = "12345".into();
            hud.opacity.readout = 1.;
            hud.opacity.header = 0.85;
            hud.opacity.rows = 1.;
            let quality = game.quality();
            let mut login = NativeLogin::new("Player")?;
            for index in 0..100 {
                let (p, prepare, pb) = measure(|| {
                    let mut p = render::prepare_gameplay(
                        &mut world,
                        &mut game.bank,
                        [960, 540],
                        1.,
                        100 + index,
                        quality,
                    )?;
                    if mode != "world" {
                        p = render::finish_hud(p, &mut game.bank, &hud)?;
                    }
                    if mode == "login" {
                        // Match native retained-picture composition, including its clone.
                        let picture = login
                            .prepare_host([960, 540], 1., true, None)?
                            .compose_over(&p)?;
                        game.bank.recycle_frame(p);
                        p = picture;
                    }
                    Ok(p)
                })?;
                if p.frame.draws.len() < 20_000 {
                    return Err("synthetic density did not reach the renderer".into());
                }
                let (_, sync, sb) = measure(|| r.sync_resources(&p.resources).map_err(Into::into))?;
                let (_, submit, rb) = measure(|| {
                    r.render(&p.frame, &view)?;
                    if let Some(overlay) = &p.overlay {
                        r.render_overlay(overlay, &view)?;
                    }
                    Ok(())
                })?;
                wait(&r)?; // Completion/pacing is deliberately outside CPU submission time.
                if index >= 40 {
                    use std::fmt::Write;
                    writeln!(
                        csv,
                        "{mode},{index},{},{},{prepare},{sync},{submit},{pb},{sb},{rb}",
                        p.draw_count(),
                        p.pixel_bytes()
                    )
                    .unwrap();
                }
                if [40, 70, 99].contains(&index) {
                    capture(&r, &texture, &dir.join(format!("{mode}-{index}.rgba")))?;
                }
                game.bank.recycle_frame(p);
            }
        }
        wait(&r)?;
        if let Some(e) = pollster::block_on(scope.pop()) {
            return Err(format!("GPU validation: {e}"));
        }
        std::fs::write(dir.join("frames.csv"), csv).map_err(|e| e.to_string())?;
        println!("PASS {name}: 60 measured frames per scene, no frame errors");
    }
    if seen.is_empty() {
        return Err("no hardware adapter exposed".into());
    }
    Ok(())
}
#[cfg(windows)]
fn main() {
    let mut args = std::env::args_os().skip(1);
    let first = args.next().expect("[--cpu-food|--zoom] new external output directory");
    let result = if first == "--cpu-food" {
        food::run()
    } else if first == "--zoom" {
        zoom::run(Path::new(
            &args.next().expect("new external output directory"),
        ))
    } else {
        run(Path::new(&first))
    };
    if let Err(e) = result {
        eprintln!("FRAME_PERF_FAILED: {e}");
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("frame_perf requires Windows: the workload uses DX12 and native login.");
    std::process::exit(1);
}
