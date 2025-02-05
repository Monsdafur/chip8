use std::fs;
use std::io::Read;
use std::time::Duration;

use sdl3::{
    event::Event,
    keyboard::Keycode,
    pixels::Color,
    rect::Rect,
    render::{Canvas, FRect},
    video::Window,
};

fn main() {
    let sdl_context = sdl3::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem.window("chip8", 1280, 640).build().unwrap();

    let mut canvas = window.into_canvas();
    let mut event_pump = sdl_context.event_pump().unwrap();

    let mut chip8 = Chip8::new(1);
    let file_path = String::from("data/1-chip8-logo.ch8");
    chip8.load(file_path);

    canvas.set_scale(20.0, 20.0).unwrap();

    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    break 'running;
                }
                _ => {}
            }
        }
        chip8.execute();
        chip8.display(&mut canvas);
        std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}

#[derive(Debug)]
enum Opcode {
    Clear,
    NormalRegistry { x: u8, n0: u8, n1: u8 },
    IndexRegistry { n0: u8, n1: u8, n2: u8 },
    Jump { n0: u8, n1: u8, n2: u8 },
    Draw { x: u8, y: u8, n: u8 },
}

struct RawOpCode {
    v0: u8,
    v1: u8,
}

impl RawOpCode {
    fn as_string(&self) -> String {
        format!("{:02X}{:02X}", self.v0, self.v1)
    }
}

struct Chip8 {
    data: [u8; 4096],
    v: [u8; 16],
    i: u16,
    start: usize,
    end: usize,
    program_counter: usize,
    render_scale: u8,
    pixels: Vec<FRect>,
}

impl Chip8 {
    fn new(render_scale: u8) -> Chip8 {
        Chip8 {
            data: [0; 4096],
            v: [0; 16],
            i: 0,
            start: 512,
            end: 512,
            program_counter: 512,
            render_scale: render_scale,
            pixels: Vec::new(),
        }
    }

    fn load(&mut self, file_path: String) {
        let mut data = Vec::new();
        let mut file = fs::File::open(file_path).unwrap();

        file.read_to_end(&mut data).unwrap();
        self.end = self.start + data.len();
        self.data[self.start..self.end].copy_from_slice(&data[..]);
    }

    fn fetch(&self) -> RawOpCode {
        RawOpCode {
            v0: self.data[self.program_counter],
            v1: self.data[self.program_counter + 1],
        }
    }

    fn decode(raw_opcode: RawOpCode) -> Opcode {
        let c00 = raw_opcode.v0 / 16;
        let c01 = raw_opcode.v0 % 16;
        let c10 = raw_opcode.v1 / 16;
        let c11 = raw_opcode.v1 % 16;
        if c00 == 0 {
            Opcode::Clear
        } else if c00 == 6 {
            Opcode::NormalRegistry {
                x: c01,
                n0: c10,
                n1: c11,
            }
        } else if c00 == 10 {
            Opcode::IndexRegistry {
                n0: c01,
                n1: c10,
                n2: c11,
            }
        } else if c00 == 13 {
            Opcode::Draw {
                x: c01,
                y: c10,
                n: c11,
            }
        } else if c00 == 1 {
            Opcode::Jump {
                n0: c01,
                n1: c10,
                n2: c11,
            }
        } else {
            unimplemented!("opcode: {}", raw_opcode.as_string());
        }
    }

    fn to_decimal(n0: u8, n1: u8, n2: u8) -> u16 {
        let u0 = n0 as u16;
        let u1 = n1 as u16;
        let u2 = n2 as u16;
        u0 * 256 + u1 * 16 + u2
    }

    fn jump(&mut self, n0: u8, n1: u8, n2: u8) {
        self.program_counter = Chip8::to_decimal(n0, n1, n2) as usize;
    }

    fn set_normal_registry(&mut self, x: u8, n0: u8, n1: u8) {
        self.v[x as usize] = Chip8::to_decimal(0, n0, n1) as u8;
    }

    fn set_index_registry(&mut self, n0: u8, n1: u8, n2: u8) {
        self.i = Chip8::to_decimal(n0, n1, n2);
    }

    fn draw(&mut self, x: u8, y: u8, n: u8) {
        let px = self.v[x as usize];
        let py = self.v[y as usize];

        for oy in 0..n {
            let idx = oy as usize + self.i as usize;
            let row = String::from(format!("{:08b} ", self.data[idx]).trim());
            for ox in 0..8 {
                let c = row.as_bytes()[ox];
                if c as u8 == 49 {
                    self.draw_pixel(px + ox as u8, py + oy);
                }
            }
        }
    }

    fn draw_pixel(&mut self, x: u8, y: u8) {
        let pixel = FRect::new(
            x as f32 * self.render_scale as f32,
            y as f32 * self.render_scale as f32,
            self.render_scale as f32,
            self.render_scale as f32,
        );
        self.pixels.push(pixel);
    }

    fn execute(&mut self) {
        let opcode = Chip8::decode(self.fetch());
        match opcode {
            Opcode::Clear => {
                self.pixels.clear();
                self.program_counter += 2;
                dbg!(opcode);
            }
            Opcode::NormalRegistry { x, n0, n1 } => {
                self.set_normal_registry(x, n0, n1);
                self.program_counter += 2;
            }
            Opcode::IndexRegistry { n0, n1, n2 } => {
                self.set_index_registry(n0, n1, n2);
                self.program_counter += 2;
            }
            Opcode::Jump { n0, n1, n2 } => {
                self.jump(n0, n1, n2);
            }
            Opcode::Draw { x, y, n } => {
                self.draw(x, y, n);
                self.program_counter += 2;
            }
        }
    }

    fn display(&self, canvas: &mut Canvas<Window>) {
        canvas.set_draw_color(Color::RGB(0, 25, 0));
        canvas.clear();
        canvas.set_draw_color(Color::RGB(0, 175, 0));
        for pixel in self.pixels.iter() {
            canvas.draw_rect(*pixel).unwrap();
        }
        canvas.present();
    }
}
