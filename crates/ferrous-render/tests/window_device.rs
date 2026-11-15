//! The compositor on a device it was lent (the window's, with *GPU preview*): the preview is drawn
//! into a texture on that device and nothing is read back for it. Its own process, since the lent
//! device is global.

use ferrous_core::{Transform, VideoFrame};
use ferrous_render::{wgpu, Layer, Renderer};

#[test]
fn preview_is_drawn_on_the_lent_device() {
    std::env::set_var("FERROUS_FORCE_GPU", "1");
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
        eprintln!("no GPU; skipping");
        return;
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).expect("device");
    ferrous_render::share_window_device(Some((adapter.clone(), device.clone(), queue.clone())));
    let (mut r, err) = Renderer::new(32, 32, true);
    if err.is_some() || !r.on_window_device() {
        eprintln!("no GPU compositor; skipping");
        return;
    }
    // Blue (BGRA 255,0,0), a 16-wide preview, no full picture wanted.
    let blue = VideoFrame::solid(32, 32, [255, 0, 0, 255]);
    r.set_bgra(false);
    r.set_preview_width(Some(16));
    let _ = r.compose(&[Layer { frame: &blue, transform: Transform::default(), filters: &[] }]);
    assert!(r.take_preview().is_none(), "nothing read back for the preview");
    let (tex, generation) = r.window_preview().expect("a preview texture");
    assert_eq!((tex.width(), tex.height()), (16, 16));
    assert!(tex.format() == wgpu::TextureFormat::Rgba8Unorm);

    // Read it back here (as the window would sample it) to check the picture.
    let first_pixel = |tex: &wgpu::Texture| -> Vec<u8> {
        let buf = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: 256 * u64::from(tex.height()), usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
        let mut enc = device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(256), rows_per_image: Some(tex.height()) } },
            wgpu::Extent3d { width: tex.width(), height: tex.height(), depth_or_array_layers: 1 },
        );
        queue.submit([enc.finish()]);
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        let px = buf.slice(..).get_mapped_range().expect("mapped")[..4].to_vec();
        px
    };
    let buf = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: 256 * 16, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(256), rows_per_image: Some(16) } },
        wgpu::Extent3d { width: 16, height: 16, depth_or_array_layers: 1 },
    );
    queue.submit([enc.finish()]);
    buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    let px = buf.slice(..).get_mapped_range().expect("mapped")[..4].to_vec();
    assert_eq!(px, [0, 0, 255, 255], "RGBA blue");

    // The same size keeps the texture; a new size makes a new one.
    let _ = r.compose(&[Layer { frame: &VideoFrame::solid(32, 32, [0, 255, 0, 255]), transform: Transform::default(), filters: &[] }]);
    assert_eq!(r.window_preview().expect("still drawn").1, generation);
    assert_eq!(first_pixel(&tex), [0, 255, 0, 255], "green now");
    // Back to a picture composed before: the preview must show it, not the last one drawn.
    let _ = r.compose(&[Layer { frame: &blue, transform: Transform::default(), filters: &[] }]);
    assert_eq!(first_pixel(&r.window_preview().expect("drawn").0), [0, 0, 255, 255], "blue again");
    r.set_preview_width(Some(24));
    let _ = r.compose(&[Layer { frame: &blue, transform: Transform::default(), filters: &[] }]);
    let (t2, g2) = r.window_preview().expect("redrawn");
    assert_eq!((t2.width(), g2), (24, generation + 1));
}
