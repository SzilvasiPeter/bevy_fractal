use bevy::{
    input::mouse::MouseWheel,
    math::DVec2,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    window::WindowResized,
};

// The maximum number of iterations to check for escape. Higher values are more detailed but slower.
const MAX_ITERATIONS: u32 = 512;

// A resource to manage the current view (position and scale) of the complex plane.
#[derive(Resource)]
struct ComplexPlaneView {
    center: DVec2,
    scale: f64, // Represents the horizontal width of the view in the complex plane
}

impl Default for ComplexPlaneView {
    fn default() -> Self {
        Self {
            // Start centered on a more interesting area
            center: DVec2::new(-0.75, 0.0),
            scale: 3.5,
        }
    }
}

// A marker component for the sprite that will display the Mandelbrot set image.
#[derive(Component)]
struct MandelbrotSprite;

// A resource to hold the handle to our dynamically generated image.
#[derive(Resource)]
struct MandelbrotImage(Handle<Image>);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_resource::<ComplexPlaneView>()
        .add_systems(Startup, (setup_camera, setup_mandelbrot_image))
        .add_systems(
            Update,
            (
                handle_panning,
                handle_zoom,
                // This system now only runs if the view has changed or the window was resized.
                draw_mandelbrot_set.run_if(
                    resource_changed::<ComplexPlaneView>.or_else(on_event::<WindowResized>()),
                ),
                on_window_resized,
            ),
        )
        .run();
}

/// Sets up a simple 2D camera.
fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
}

/// Creates the initial Image asset and spawns a sprite to display it.
fn setup_mandelbrot_image(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    windows: Query<&Window>,
) {
    let window = windows.single();
    let window_size = Extent3d {
        width: window.physical_width(),
        height: window.physical_height(),
        depth_or_array_layers: 1,
    };

    let mut image = Image::new_fill(
        window_size,
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        default(),
    );
    image.texture_descriptor.usage = bevy::render::render_resource::TextureUsages::COPY_DST
        | bevy::render::render_resource::TextureUsages::TEXTURE_BINDING;

    let image_handle = images.add(image);
    commands.insert_resource(MandelbrotImage(image_handle.clone()));

    commands.spawn((
        SpriteBundle {
            texture: image_handle,
            ..default()
        },
        MandelbrotSprite,
    ));
}

/// Handles panning the view by clicking and dragging the mouse.
fn handle_panning(
    mut view: ResMut<ComplexPlaneView>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut cursor_events: EventReader<CursorMoved>,
    windows: Query<&Window>,
    mut last_pos: Local<Option<Vec2>>,
) {
    if mouse_buttons.pressed(MouseButton::Left) {
        let window = windows.single();
        if let Some(current_pos) = cursor_events.read().last().map(|e| e.position) {
            if let Some(last_pos_val) = *last_pos {
                let delta_pixels = current_pos - last_pos_val;
                // Convert pixel delta to complex plane delta
                let delta_complex = DVec2::new(delta_pixels.x as f64, -delta_pixels.y as f64)
                    * (view.scale / window.width() as f64);
                view.center -= delta_complex;
            }
            *last_pos = Some(current_pos);
        }
    } else {
        *last_pos = None;
    }
}

/// Handles zooming the view with the mouse scroll wheel.
fn handle_zoom(
    mut view: ResMut<ComplexPlaneView>,
    mut scroll_events: EventReader<MouseWheel>,
    windows: Query<&Window>,
) {
    for event in scroll_events.read() {
        let window = windows.single();
        if let Some(cursor_pos) = window.cursor_position() {
            // Convert cursor position to complex plane coordinates
            let aspect_ratio = window.width() as f64 / window.height() as f64;
            let complex_height = view.scale / aspect_ratio;

            let complex_x = view.center.x - view.scale / 2.0
                + (cursor_pos.x as f64 / window.width() as f64) * view.scale;
            let complex_y = view.center.y + complex_height / 2.0
                - (cursor_pos.y as f64 / window.height() as f64) * complex_height;
            let cursor_complex = DVec2::new(complex_x, complex_y);

            // Zoom factor
            let zoom_factor = 1.0 - event.y as f64 * 0.1;
            let new_scale = view.scale * zoom_factor;

            // Adjust center to keep the point under the cursor stationary
            view.center = cursor_complex - (cursor_complex - view.center) * zoom_factor;
            view.scale = new_scale;
        }
    }
}

/// Resizes the underlying image asset when the window is resized.
fn on_window_resized(
    mut events: EventReader<WindowResized>,
    mut images: ResMut<Assets<Image>>,
    mandelbrot_image: Res<MandelbrotImage>,
) {
    if let Some(event) = events.read().last() {
        if let Some(image) = images.get_mut(&mandelbrot_image.0) {
            let new_size = Extent3d {
                width: event.width as u32,
                height: event.height as u32,
                depth_or_array_layers: 1,
            };
            image.resize(new_size);
        }
    }
}

/// The core system that calculates and draws the Mandelbrot set onto the image.
fn draw_mandelbrot_set(
    mut images: ResMut<Assets<Image>>,
    mandelbrot_image: Res<MandelbrotImage>,
    view: Res<ComplexPlaneView>,
) {
    if let Some(image) = images.get_mut(&mandelbrot_image.0) {
        let width = image.texture_descriptor.size.width;
        let height = image.texture_descriptor.size.height;
        let data: &mut [u8] = image.data.as_mut();

        // Calculate the bounds of the complex plane to render based on the view
        let aspect_ratio = width as f64 / height as f64;
        let y_scale = view.scale / aspect_ratio;
        let x_min = view.center.x - view.scale / 2.0;
        let x_max = view.center.x + view.scale / 2.0;
        let y_min = view.center.y - y_scale / 2.0;
        let y_max = view.center.y + y_scale / 2.0;

        // Iterate over every pixel in the image buffer.
        for y in 0..height {
            for x in 0..width {
                let cx = map_range(x as f64, 0.0, (width - 1) as f64, x_min, x_max);
                let cy = map_range(y as f64, 0.0, (height - 1) as f64, y_max, y_min); // Y is inverted

                let mut zx = 0.0;
                let mut zy = 0.0;
                let mut i = 0;

                while zx * zx + zy * zy < 4.0 && i < MAX_ITERATIONS {
                    let temp_zx = zx * zx - zy * zy + cx;
                    zy = 2.0 * zx * zy + cy;
                    zx = temp_zx;
                    i += 1;
                }

                let color = if i == MAX_ITERATIONS {
                    [0, 0, 0, 255]
                } else {
                    let n = i as f32 / MAX_ITERATIONS as f32;
                    let r = (9.0 * (1.0 - n) * n * n * n * 255.0) as u8;
                    let g = (15.0 * (1.0 - n) * (1.0 - n) * n * n * 255.0) as u8;
                    let b = (8.5 * (1.0 - n) * (1.0 - n) * (1.0 - n) * n * 255.0) as u8;
                    [r, g, b, 255]
                };

                let pixel_index = ((y * width) + x) as usize * 4;
                data[pixel_index..pixel_index + 4].copy_from_slice(&color);
            }
        }
    }
}

/// A helper function to map a value from one range to another.
fn map_range(val: f64, in_min: f64, in_max: f64, out_min: f64, out_max: f64) -> f64 {
    (val - in_min) * (out_max - out_min) / (in_max - in_min) + out_min
}
