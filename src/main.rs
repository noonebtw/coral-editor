use graphics::math::Matrix2d;
use image::{imageops, RgbaImage};
use log::info;

use glutin_window::GlutinWindow as Window;
use opengl_graphics::{GlGraphics, OpenGL, Texture, TextureSettings};
use piston::{
    event_loop::{EventSettings, Events},
    Button, ButtonState, MouseButton, MouseCursorEvent,
};
use piston::{
    input::{RenderArgs, RenderEvent, UpdateArgs, UpdateEvent},
    ButtonEvent,
};
use piston::{window::WindowSettings, ButtonArgs};
use vecmath::{mat2x3_id, mat2x3_inv, row_mat2x3_transform_pos2};

pub struct App {
    gl: GlGraphics, // OpenGL drawing backend.
    image: RgbaImage,
    texture: Texture,
    area_selection: (Option<[f64; 2]>, Option<[f64; 2]>),
    last_mouse_pos: Option<[f64; 2]>,
}

impl App {
    fn new(gl: GlGraphics) -> Self {
        let image = image::io::Reader::open("bladerunner.jpg")
            .unwrap()
            .decode()
            .unwrap()
            .to_rgba8();

        let texture = Texture::from_image(&image, &TextureSettings::new());

        Self {
            gl,
            image,
            texture,
            area_selection: (None, None),
            last_mouse_pos: None,
        }
    }

    fn load_texture(&mut self) {
        self.texture = Texture::from_image(&self.image, &TextureSettings::new());
    }

    fn render(&mut self, args: &RenderArgs) {
        let Self {
            gl,
            image,
            texture,
            area_selection,
            last_mouse_pos,
            ..
        } = self;

        use graphics::*;

        const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
        const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

        let (window_width, window_height) = (args.window_size[0], args.window_size[1]);
        let (x, y) = (args.window_size[0] / 2.0, args.window_size[1] / 2.0);

        let (image_width, image_height) = texture.get_size();

        let (ratio_width, ratio_height) = (
            window_width / image_width as f64,
            window_height / image_height as f64,
        );

        let ratio = f64::min(ratio_width, ratio_height);

        let trans = (mat2x3_id() as Matrix2d)
            .trans(x, y)
            .scale(ratio, ratio)
            .trans(
                0.0 - (image_width / 2) as f64,
                0.0 - (image_height / 2) as f64,
            );

        gl.draw(args.viewport(), |ctx, gl| {
            // Clear the screen.
            clear(GREEN, gl);

            let trans = ctx.transform.append_transform(trans);

            // Draw a box rotating around the middle of the screen.
            graphics::image(texture, trans, gl);

            // draw selection box
            if let (Some(start), Some(end)) = (area_selection.0, last_mouse_pos) {
                let a = start;
                let c = *end;
                let b = [c[0], a[1]];
                let d = [a[0], c[1]];

                graphics::line_from_to(BLACK, 1.0, a, b, ctx.transform, gl);
                graphics::line_from_to(BLACK, 1.0, b, c, ctx.transform, gl);
                graphics::line_from_to(BLACK, 1.0, c, d, ctx.transform, gl);
                graphics::line_from_to(BLACK, 1.0, d, a, ctx.transform, gl);
            }
        });

        if let (Some(start), Some(end)) = self.area_selection {
            info!("Crop: {:#?}", (start, end));

            let (start, end) = {
                (
                    row_mat2x3_transform_pos2(mat2x3_inv(trans), start),
                    row_mat2x3_transform_pos2(mat2x3_inv(trans), end),
                )
            };

            info!("Crop: {:#?}", (start, end));

            let (start, size) = {
                (
                    (start[0] as u32, start[1] as u32),
                    ((end[0] - start[0]) as u32, (end[1] - start[1]) as u32),
                )
            };

            // sanitize
            let (start, size) = {
                (
                    (std::cmp::max(0, start.0), std::cmp::max(0, start.1)),
                    (std::cmp::max(0, size.0), std::cmp::max(0, size.1)),
                )
            };

            self.image =
                imageops::crop_imm(&self.image, start.0, start.1, size.0, size.1).to_image();

            self.load_texture();

            self.area_selection = (None, None);
        }
    }

    fn input(&mut self, button: Option<ButtonArgs>, mouse: Option<[f64; 2]>) {
        if let Some(b) = button {
            info!("button: {:?}", b);

            if b.button == Button::Mouse(MouseButton::Left) && b.state == ButtonState::Press {
                if let Some(mouse) = self.last_mouse_pos {
                    self.area_selection.0 = Some(mouse);
                    self.area_selection.1 = None;
                }
            }

            if b.button == Button::Mouse(MouseButton::Left) && b.state == ButtonState::Release {
                if let Some(mouse) = self.last_mouse_pos {
                    self.area_selection.1 = Some(mouse);
                }
            }
        }

        if let Some(m) = mouse {
            self.last_mouse_pos = Some(m);
        }
    }

    fn update(&mut self, _args: &UpdateArgs) {}
}

fn main() {
    simple_logger::SimpleLogger::new().init().unwrap();

    // Change this to OpenGL::V2_1 if not working.
    let opengl = OpenGL::V3_2;

    // Create an Glutin window.
    let mut window: Window = WindowSettings::new("spinning-square", [200, 200])
        .graphics_api(opengl)
        .exit_on_esc(true)
        .build()
        .unwrap();

    // Create a new game and run it.
    let mut app = App::new(GlGraphics::new(opengl));

    let mut events = Events::new(EventSettings::new());
    while let Some(e) = events.next(&mut window) {
        if let Some(args) = e.render_args() {
            app.render(&args);
        }

        app.input(e.button_args(), e.mouse_cursor_args());

        if let Some(args) = e.update_args() {
            app.update(&args);
        }
    }
}