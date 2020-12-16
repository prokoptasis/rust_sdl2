extern crate sdl2;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Point;
use sdl2::rect::Rect;
use sdl2::render::TextureQuery;
use std::collections::HashSet;
use std::env;
use std::time::Duration;

// Audio
use sdl2::audio::{AudioCVT, AudioCallback, AudioSpecDesired, AudioSpecWAV};
use std::borrow::Cow;
use std::path::{Path, PathBuf};

static SCR_WDTH: u32 = 1024;
static SCR_HGHT: u32 = 768;

// handle the annoying Rect i32
macro_rules! rect(
    ($x:expr, $y:expr, $w:expr, $h:expr) => (
        Rect::new($x as i32, $y as i32, $w as u32, $h as u32)
    )
);

// Scale fonts to a reasonable size when they're too big (though they might look less smooth)
fn get_centered_rect(rect_width: u32, rect_height: u32, cons_width: u32, cons_height: u32) -> Rect {
    let wr = rect_width as f32 / cons_width as f32;
    let hr = rect_height as f32 / cons_height as f32;

    let (w, h) = if wr > 1f32 || hr > 1f32 {
        if wr > hr {
            println!("Scaling down! The text will look worse!");
            let h = (rect_height as f32 / wr) as i32;
            (cons_width as i32, h)
        } else {
            println!("Scaling down! The text will look worse!");
            let w = (rect_width as f32 / hr) as i32;
            (w, cons_height as i32)
        }
    } else {
        (rect_width as i32, rect_height as i32)
    };

    let cx = (SCR_WDTH as i32 - w) / 2;
    let cy = (SCR_HGHT as i32 - h) / 2;
    rect!(cx, cy, w, h)
}

fn run(font_path: &Path) -> Result<(), String> {
    let sdl_context = sdl2::init()?;
    let video_subsys = sdl_context.video()?;
    let ttf_context = sdl2::ttf::init().map_err(|e| e.to_string())?;

    let window = video_subsys
        .window("SDL2_TTF Example", SCR_WDTH, SCR_HGHT)
        .position_centered()
        .opengl()
        .build()
        .map_err(|e| e.to_string())?;

    let mut canvas = window.into_canvas().build().map_err(|e| e.to_string())?;
    let texture_creator = canvas.texture_creator();

    // Load a font
    let mut font = ttf_context.load_font(font_path, 128)?;
    font.set_style(sdl2::ttf::FontStyle::BOLD);

    // render a surface, and convert it to a texture bound to the canvas
    let surface = font
        .render("WalaBalaWWal!")
        .blended(Color::RGBA(255, 0, 0, 255))
        .map_err(|e| e.to_string())?;
    let texture = texture_creator
        .create_texture_from_surface(&surface)
        .map_err(|e| e.to_string())?;

    canvas.set_draw_color(Color::RGBA(195, 217, 255, 255));
    canvas.clear();

    let TextureQuery { width, height, .. } = texture.query();

    // If the example text is too big for the screen, downscale it (and center irregardless)
    let padding = 64;
    let target = get_centered_rect(width, height, SCR_WDTH - padding, SCR_HGHT - padding);

    canvas.copy(&texture, None, Some(target))?;
    canvas.present();

    'mainloop: loop {
        for event in sdl_context.event_pump()?.poll_iter() {
            match event {
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                }
                | Event::Quit { .. } => break 'mainloop,
                _ => {}
            }
        }
    }

    Ok(())
}

struct Sound {
    data: Vec<u8>,
    volume: f32,
    pos: usize,
}

impl AudioCallback for Sound {
    type Channel = u8;

    fn callback(&mut self, out: &mut [u8]) {
        for dst in out.iter_mut() {
            // With channel type u8 the "silence" value is 128 (middle of the 0-2^8 range) so we need
            // to both fill in the silence and scale the wav data accordingly. Filling the silence
            // once the wav is finished is trivial, applying the volume is more tricky. We need to:
            // * Change the range of the values from [0, 255] to [-128, 127] so we can multiply
            // * Apply the volume by multiplying, this gives us range [-128*volume, 127*volume]
            // * Move the resulting range to a range centered around the value 128, the final range
            //   is [128 - 128*volume, 128 + 127*volume] – scaled and correctly positioned
            //
            // Using value 0 instead of 128 would result in clicking. Scaling by simply multiplying
            // would not give correct results.
            let pre_scale = *self.data.get(self.pos).unwrap_or(&128);
            let scaled_signed_float = (pre_scale as f32 - 128.0) * self.volume;
            let scaled = (scaled_signed_float + 128.0) as u8;
            *dst = scaled;
            self.pos += 1;
        }
    }
}

fn main() -> Result<(), String> {
    let mut c_pos_x = 300;
    let mut pos_x = 0;
    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;

    let window = video_subsystem
        .window("SDL2", SCR_WDTH, SCR_HGHT)
        .position(250, 150)
        // .position_centered()
        .build()
        .map_err(|e| e.to_string())?;

    let mut canvas = window
        .into_canvas()
        .accelerated()
        .build()
        .map_err(|e| e.to_string())?;
    let texture_creator = canvas.texture_creator();

    canvas.set_draw_color(sdl2::pixels::Color::RGBA(255, 255, 255, 255));

    let mut timer = sdl_context.timer()?;

    let mut event_pump = sdl_context.event_pump()?;

    // animation sheet and extras are available from
    // https://opengameart.org/content/a-platformer-in-the-forest
    let temp_surface =
        sdl2::surface::Surface::load_bmp(Path::new("assets/Adventurer-Sprite-Sheet-v1.1.bmp"))?;
    let texture = texture_creator
        .create_texture_from_surface(&temp_surface)
        .map_err(|e| e.to_string())?;

    let frames_per_anim = 10;
    let sprite_tile_size = (32, 32);

    // Baby - walk animation
    let mut source_rect_0 = Rect::new(0, 128, sprite_tile_size.0, sprite_tile_size.0);
    let mut dest_rect_0 = Rect::new(0, 0, sprite_tile_size.0 * 8, sprite_tile_size.0 * 8);
    dest_rect_0.center_on(Point::new(300, 350));

    let wav_file: Cow<'static, Path> = match std::env::args().nth(1) {
        None => Cow::from(Path::new("./assets/sound3.wav")),
        Some(s) => Cow::from(PathBuf::from(s)),
    };
    let audio_subsystem = sdl_context.audio()?;

    let desired_spec = AudioSpecDesired {
        freq: Some(44_100),
        channels: Some(1), // mono
        samples: None,     // default
    };

    let mut device = audio_subsystem.open_playback(None, &desired_spec, |spec| {
        let wav = AudioSpecWAV::load_wav(wav_file).expect("Could not load test WAV file");

        let cvt = AudioCVT::new(
            wav.format,
            wav.channels,
            wav.freq,
            spec.format,
            spec.channels,
            spec.freq,
        )
        .expect("Could not convert WAV file");

        let data = cvt.convert(wav.buffer().to_vec());

        // initialize the audio callback
        Sound {
            data: data,
            volume: 0.25,
            pos: 0,
        }
    })?;
    let fnt_path: &Path = Path::new("./assets/d2coding.ttc");
    // let fnt_path: &Path = Path::new("./assets/tahoma.ttf");

    // let video_subsys = sdl_context.video()?;
    let ttf_context = sdl2::ttf::init().map_err(|e| e.to_string())?;

    // let window = video_subsys
    //     .window("SDL2_TTF Example", SCR_WDTH, SCR_HGHT)
    //     .position_centered()
    //     .opengl()
    //     .build()
    //     .map_err(|e| e.to_string())?;

    // let mut canvas = window.into_canvas().build().map_err(|e| e.to_string())?;
    let texture_creator = canvas.texture_creator();

    // // Load a font
    let mut font = ttf_context.load_font(fnt_path, 128)?;
    font.set_style(sdl2::ttf::FontStyle::BOLD);

    // render a surface, and convert it to a texture bound to the canvas
    let surface = font
        .render("안녕???")
        .blended(Color::RGBA(255, 0, 0, 255))
        .map_err(|e| e.to_string())?;
    let fnt_texture = texture_creator
        .create_texture_from_surface(&surface)
        .map_err(|e| e.to_string())?;

    let mut prev_buttons = HashSet::new();
    let mut running = true;
    while running {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    running = false;
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Right),
                    ..
                } => {
                    pos_x = 32;
                    // Start playback
                    device.resume();
                    {
                        let mut lock = device.lock();
                        lock.pos = 0;
                    }
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Left),
                    ..
                } => {
                    pos_x = -32;
                    // Start playback
                    device.resume();
                    {
                        let mut lock = device.lock();
                        lock.pos = 0;
                    }
                }
                Event::KeyUp { .. } => {
                    pos_x = 0;
                }
                _ => {}
            }
        }

        let ticks = timer.ticks() as i32;

        // set the current frame for time
        source_rect_0.set_x(32 * ((ticks / 100) % frames_per_anim));
        c_pos_x = c_pos_x + pos_x;
        dest_rect_0.set_x(c_pos_x);

        canvas.clear();
        // copy the frame to the canvas
        canvas.copy_ex(
            &texture,
            Some(source_rect_0),
            Some(dest_rect_0),
            0.0,
            None,
            false,
            false,
        )?;

        // get a mouse state
        let state = event_pump.mouse_state();

        // Create a set of pressed Keys.
        let buttons = state.pressed_mouse_buttons().collect();

        // Get the difference between the new and old sets.
        let new_buttons = &buttons - &prev_buttons;
        let old_buttons = &prev_buttons - &buttons;

        if !new_buttons.is_empty() || !old_buttons.is_empty() {
            println!(
                "X = {:?}, Y = {:?} : {:?} -> {:?}",
                state.x(),
                state.y(),
                new_buttons,
                old_buttons
            );
        }

        prev_buttons = buttons;

        let TextureQuery { width, height, .. } = texture.query();

        // If the example text is too big for the screen, downscale it (and center irregardless)
        let padding = 64;
        let target = get_centered_rect(48, 48, SCR_WDTH - padding, SCR_HGHT - padding);
        canvas.copy(&fnt_texture, None, Some(target))?;
        // canvas.present();
        canvas.present();

        std::thread::sleep(Duration::from_millis(100));
    }

    Ok(())
}
