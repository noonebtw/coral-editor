use std::{
    io::{Cursor, Read, Result},
    path::PathBuf,
};

use glutin_window::GlutinWindow;
use graphics::math::Matrix2d;
use image::{imageops, png::PngDecoder, DynamicImage, ImageOutputFormat, ImageResult, RgbaImage};
use log::{debug, error, info, warn};

use opengl_graphics::{GlGraphics, OpenGL, Texture, TextureSettings};
use piston::{
    event_loop::{EventSettings, Events},
    Button, ButtonState, Key, MouseButton, MouseCursorEvent, Window,
};
use piston::{
    input::{RenderArgs, RenderEvent, UpdateArgs, UpdateEvent},
    ButtonEvent,
};
use piston::{window::WindowSettings, ButtonArgs};
use vecmath::{mat2x3_id, mat2x3_inv, row_mat2x3_transform_pos2};

pub struct App {
    config: Config,
    gl: GlGraphics, // OpenGL drawing backend.
    image: RgbaImage,
    texture: Texture,
    area_selection: (Option<[f64; 2]>, Option<[f64; 2]>),
    last_mouse_pos: Option<[f64; 2]>,
}

impl App {
    fn new(gl: GlGraphics, config: Config) -> Self {
        let image = config.open_image().unwrap();

        let texture = Texture::from_image(&image, &TextureSettings::new());

        Self {
            config,
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
            texture,
            area_selection,
            last_mouse_pos,
            ..
        } = self;

        use graphics::*;

        const BACKGROUND: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
        const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

        let (window_width, window_height) = (args.window_size[0], args.window_size[1]);
        let (x, y) = (args.window_size[0] / 2.0, args.window_size[1] / 2.0);

        let (image_width, image_height) = texture.get_size();

        let (ratio_width, ratio_height) = (
            window_width / image_width as f64,
            window_height / image_height as f64,
        );

        let ratio = f64::min(ratio_width, ratio_height) * 0.95;

        let trans = (mat2x3_id() as Matrix2d)
            .trans(x, y)
            .scale(ratio, ratio)
            .trans(
                0.0 - (image_width / 2) as f64,
                0.0 - (image_height / 2) as f64,
            );

        gl.draw(args.viewport(), |ctx, gl| {
            // Clear the screen.
            clear(BACKGROUND, gl);

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
            let (a, b) = {
                (
                    row_mat2x3_transform_pos2(mat2x3_inv(trans), start),
                    row_mat2x3_transform_pos2(mat2x3_inv(trans), end),
                )
            };

            // sanitize
            let (a, b) = {
                use std::cmp::min;
                (
                    (
                        min(image_width, f64::max(0.0, a[0]) as u32),
                        min(image_height, f64::max(0.0, a[1]) as u32),
                    ),
                    (
                        min(image_width, f64::max(0.0, b[0]) as u32),
                        min(image_height, f64::max(0.0, b[1]) as u32),
                    ),
                )
            };

            let (start, size) = {
                use std::cmp::min;

                let start = (min(a.0, b.0), min(a.1, b.1));

                // u32 abs() when?
                let size = (
                    a.0.checked_sub(b.0)
                        .unwrap_or_else(|| b.0.checked_sub(a.0).unwrap()),
                    b.1.checked_sub(a.1)
                        .unwrap_or_else(|| a.1.checked_sub(b.1).unwrap()),
                );

                (start, size)
            };

            info!("Crop: {:#?}", (start, size));

            self.image =
                imageops::crop_imm(&self.image, start.0, start.1, size.0, size.1).to_image();

            self.load_texture();

            self.area_selection = (None, None);
        }
    }

    fn input(
        &mut self,
        window: &mut GlutinWindow,
        button: Option<ButtonArgs>,
        mouse: Option<[f64; 2]>,
    ) {
        if let Some(b) = button {
            if b.button == Button::Mouse(MouseButton::Left) && b.state == ButtonState::Press {
                if let Some(mouse) = self.last_mouse_pos {
                    self.area_selection.0 = Some(mouse);
                    self.area_selection.1 = None;
                }
            }

            if b.button == Button::Mouse(MouseButton::Left) && b.state == ButtonState::Release {
                if let (Some(mouse), Some(_)) = (self.last_mouse_pos, self.area_selection.0) {
                    self.area_selection.1 = Some(mouse);
                }
            }

            if b.button == Button::Keyboard(Key::Escape) && b.state == ButtonState::Release {
                if self.area_selection.0.is_some() {
                    self.area_selection = (None, None);
                } else {
                    info!("saving image..");
                    let _ = self
                        .config
                        .save_image(DynamicImage::ImageRgba8(self.image.clone()))
                        .map_err(|e| error!("Error while saving image: {:#?}", e));

                    window.set_should_close(true);
                }
            }
        }

        if let Some(m) = mouse {
            self.last_mouse_pos = Some(m);
        }
    }

    fn update(&mut self, _args: &UpdateArgs) {}
}

#[derive(Debug)]
struct Config {
    input_file: Option<PathBuf>,
    output_file: Option<PathBuf>,
    graphical: bool,
}

impl Config {
    fn open_image(&self) -> ImageResult<RgbaImage> {
        match &self.input_file {
            Some(path) => Ok(image::io::Reader::open(&path)?.decode()?.to_rgba8()),
            None => {
                info!("reading image data from stdin..");

                let stdin = std::io::stdin();
                let mut buf = Vec::new();
                stdin.lock().read_to_end(&mut buf)?;

                Ok(image::io::Reader::new(Cursor::new(buf))
                    .with_guessed_format()?
                    .decode()?
                    .to_rgba8())
            }
        }
    }

    fn save_image(&self, image: DynamicImage) -> ImageResult<()> {
        match &self.output_file {
            Some(path) => {
                info!("saving as {}", path.to_string_lossy());
                image.save(path)?;
            }
            None => {
                if !atty::is(atty::Stream::Stdout) {
                    let stdout = std::io::stdout();

                    image.write_to(&mut stdout.lock(), ImageOutputFormat::Png)?;
                } else {
                    warn!("stdout is a tty, aborting printing binary..");
                }
            }
        };

        Ok(())
    }
}

fn parse_commandline() -> Config {
    let matches = clap::App::new(std::env!("CARGO_BIN_NAME"))
        .version(std::env!("CARGO_PKG_VERSION"))
        .author("Janis Böhm (No One)")
        .about("Coral takes screenshots")
        .arg(
            clap::Arg::with_name("input_file")
                .short("i")
                .aliases(&["f", "i", "input"])
                .long("file")
                .value_name("input_file")
                .help("input file name"),
        )
        .arg(
            clap::Arg::with_name("output_file")
                .short("o")
                .long("output")
                .value_name("output_file")
                .help("output file name; if not specified, the image will be printed to `stdout`"),
        )
        .arg(
            clap::Arg::with_name("quiet")
                .short("q")
                .long("quiet")
                .takes_value(false)
                .help("silent execution"),
        )
        .arg(
            clap::Arg::with_name("gui")
                .short("g")
                .long("graphical")
                .takes_value(false)
                .help("Enables GUI to edit image; if omitted the default behaviour is to write `input_file` to `output_file`"),
        ).get_matches();

    if !matches.is_present("quiet") {
        simple_logger::SimpleLogger::new().init().unwrap();
    }

    Config {
        input_file: matches.value_of("input_file").map(|s| s.into()),
        output_file: matches.value_of("output_file").map(|s| s.into()),
        graphical: matches.is_present("gui"),
    }
}

fn run_graphical(config: Config) {
    let opengl = OpenGL::V3_2;

    // Create an Glutin window.
    let mut window = WindowSettings::new(std::env!("CARGO_BIN_NAME"), [200, 200])
        .graphics_api(opengl)
        .transparent(true)
        .exit_on_esc(false)
        .build()
        .unwrap();

    // Create a new game and run it.
    let mut app = App::new(GlGraphics::new(opengl), config);

    let mut events = Events::new(EventSettings::new());
    while let Some(e) = events.next(&mut window) {
        if let Some(args) = e.render_args() {
            app.render(&args);
        }

        app.input(&mut window, e.button_args(), e.mouse_cursor_args());

        if let Some(args) = e.update_args() {
            app.update(&args);
        }
    }
}

fn run_cli(config: Config) {
    info!("CLI runner.");

    let image = config.open_image().unwrap();

    let _ = config
        .save_image(DynamicImage::ImageRgba8(image))
        .map_err(|e| error!("Error while saving image: {:#?}", e));
}

fn main() -> Result<()> {
    let config = parse_commandline();
    debug!("config: {:#?}", config);

    if config.graphical {
        run_graphical(config);
    } else {
        run_cli(config);
    }

    info!("exiting successfully");
    Ok(())
}
