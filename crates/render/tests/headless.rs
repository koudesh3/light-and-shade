//! Renders real frames offscreen. Skips when the machine has no GPU adapter.

use las_core::{Light, Project, View};
use las_render::{Frame, Mode, Renderer, TARGET_FORMAT};

const SIZE: u32 = 512;

fn gpu() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).ok()?;
    pollster::block_on(adapter.request_device(&Default::default())).ok()
}

fn draw(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    renderer: &Renderer,
    p: &Project,
    light: Light,
    mode: Mode,
) -> Vec<u8> {
    let size = wgpu::Extent3d {
        width: SIZE,
        height: SIZE,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TARGET_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (SIZE * SIZE * 4) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    let target = [SIZE as f32; 2];
    let frame = Frame {
        transform: View::default().transform(target, [p.width as f32, p.height as f32]),
        light: light.direction(),
        ambient: 0.34,
        strength: 0.95,
        sheen: 0.08,
        mode,
        linear: false,
        background: [1.0, 1.0, 1.0],
        visible: &[],
    };
    renderer.render(
        queue,
        &mut encoder,
        &texture.create_view(&Default::default()),
        &frame,
    );
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(SIZE * 4),
                rows_per_image: None,
            },
        },
        size,
    );
    queue.submit([encoder.finish()]);
    buffer.slice(..).map_async(wgpu::MapMode::Read, |r| r.unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    buffer.slice(..).get_mapped_range().unwrap().to_vec()
}

#[test]
fn lighting_follows_the_light() {
    let Some((device, queue)) = gpu() else { return };
    let p = Project::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../projects/pothos")).unwrap();
    let mut r = Renderer::new(&device);
    r.load(&device, &queue, &p).unwrap();

    let left = draw(
        &device,
        &queue,
        &r,
        &p,
        Light {
            x: -1.0,
            y: 0.3,
            height: 0.6,
        },
        Mode::Lit,
    );
    let right = draw(
        &device,
        &queue,
        &r,
        &p,
        Light {
            x: 1.0,
            y: 0.3,
            height: 0.6,
        },
        Mode::Lit,
    );
    let painted = |px: &[u8]| {
        px.chunks(4)
            .filter(|c| c[0] < 250 || c[1] < 250 || c[2] < 250)
            .count()
    };
    assert!(
        painted(&left) > (SIZE * SIZE) as usize / 10,
        "the painting should cover the frame"
    );
    let changed = left
        .chunks(4)
        .zip(right.chunks(4))
        .filter(|(a, b)| a[1].abs_diff(b[1]) > 16)
        .count();
    assert!(
        changed > painted(&left) / 4,
        "moving the light should reshade the leaves"
    );

    if let Ok(dir) = std::env::var("LAS_STILLS") {
        for (name, mode) in [
            ("lit", Mode::Lit),
            ("normals", Mode::Normals),
            ("original", Mode::Original),
        ] {
            let px = draw(
                &device,
                &queue,
                &r,
                &p,
                Light {
                    x: -0.55,
                    y: 0.5,
                    height: 0.55,
                },
                mode,
            );
            image::save_buffer(
                format!("{dir}/{name}.png"),
                &px,
                SIZE,
                SIZE,
                image::ColorType::Rgba8,
            )
            .unwrap();
        }
    }
}
