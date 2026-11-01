//! The native event gate is allocation-free without an owning egui screen.
#![cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
use slither_wgpu_slice::egui_native::NativeEguiHost;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{DeviceId, ElementState, WindowEvent},
    keyboard::{Key, NamedKey},
};

struct Counted;
thread_local! {
    static TRACK: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
}
// SAFETY: every allocation operation is forwarded unchanged to System. The
// thread-local counters themselves use const initialization without allocation.
unsafe impl GlobalAlloc for Counted {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        TRACK.with(|track| {
            if track.get() {
                COUNT.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        TRACK.with(|track| {
            if track.get() {
                COUNT.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Counted = Counted;

fn closed_callbacks(host: &mut NativeEguiHost) {
    COUNT.with(|count| count.set(0));
    TRACK.with(|track| track.set(true));
    for i in 0..1000 {
        for event in [
            WindowEvent::RedrawRequested,
            WindowEvent::CursorMoved {
                device_id: DeviceId::dummy(),
                position: PhysicalPosition::new(i as f64, 200.),
            },
            WindowEvent::Resized(PhysicalSize::new(960, 540)),
        ] {
            let response = host.window_event(&event, 1.75, || {
                panic!("inactive adapter reached the window")
            });
            assert!(!response.consumed && !response.repaint);
            assert!(
                host.take_window_input(|| panic!("inactive drain reached the window"))
                    .is_none()
            );
        }
        host.set_active(false);
        host.synthetic_key(&Key::Named(NamedKey::Space), ElementState::Pressed);
        host.synthetic_key(&Key::Named(NamedKey::Space), ElementState::Released);
        host.cancel_pointer();
        host.step(i as f64);
    }
    TRACK.with(|track| track.set(false));
    assert_eq!(COUNT.with(Cell::get), 0);
}

#[test]
fn closed_callbacks_allocate_nothing_before_or_after_first_use() {
    let mut host = NativeEguiHost::default();
    closed_callbacks(&mut host);
    assert!(!host.initialized());
    host.set_active(true);
    host.frame([960, 540], 1., &[], |ctx, _| {
        egui::Area::new(egui::Id::unique("authored-test")).show(ctx, |ui| {
            ui.label("Authored test");
        });
    });
    host.check_error().unwrap();
    assert!(host.initialized());
    host.set_active(false);
    closed_callbacks(&mut host);
}

#[test]
#[ignore = "explicit finite offscreen hardware-GPU lifecycle check"]
fn shared_context_uploads_resizes_and_reopens_without_gpu_errors() {
    use slither_wgpu_slice::{
        egui_theme,
        gpu::{Renderer, instance},
    };
    let gpu = pollster::block_on(Renderer::new(
        &instance(),
        None,
        wgpu::TextureFormat::Rgba8Unorm,
    ))
    .unwrap();
    let validation = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut host = NativeEguiHost::default();
    let marker = egui::Id::unique("retained-host-marker");
    for (index, (extent, dpr)) in [([960, 540], 1.), ([720, 1280], 2.), ([960, 540], 1.)]
        .into_iter()
        .enumerate()
    {
        host.set_active(true);
        let context = host.context().unwrap();
        if index == 0 {
            context.data_mut(|data| data.insert_temp(marker, 42_u32));
        }
        assert_eq!(context.data(|data| data.get_temp::<u32>(marker)), Some(42));
        host.frame(extent, dpr, &[], |ctx, _| {
            egui::Area::new(egui::Id::unique(("authored-screen", index))).show(ctx, |ui| {
                egui_theme::panel_frame().show(ui, |ui| {
                    egui_theme::title(
                        ui,
                        if index == 0 {
                            "Authored first screen"
                        } else {
                            "Authored next screen"
                        },
                    );
                });
            });
        });
        host.check_error().unwrap();
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("authored menu lifecycle"),
            size: wgpu::Extent3d {
                width: extent[0],
                height: extent[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        host.paint(&gpu, &view);
        gpu.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(10)),
            })
            .unwrap();
        host.set_active(false);
        host.frame(extent, dpr, &[], |_, _| panic!("closed host drew a screen"));
        host.paint(&gpu, &view);
    }
    drop(host);
    assert!(pollster::block_on(validation.pop()).is_none());
}
