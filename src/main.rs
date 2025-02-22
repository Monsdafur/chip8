use clap::Parser;
use std::io::Read;
use std::time::Duration;
use std::{fs, path::PathBuf};

use sdl3::{
    event::Event, keyboard::Keycode, pixels::Color, rect::Point, render::Canvas, video::Window,
};

fn main() {
    let args = Cli::parse();
    let mut chip8 = Chip8::new();
    chip8.load(args.path);

    let sdl_context = sdl3::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem.window("chip8", 1280, 640).build().unwrap();

    let mut canvas = window.into_canvas();
    let mut event_pump = sdl_context.event_pump().unwrap();
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

#[derive(Parser)]
struct Cli {
    path: PathBuf,
}

#[derive(Debug)]
enum Opcode {
    Clear,
    Return,

    NormalRegistry { x: u8, n0: u8, n1: u8 },
    IndexRegistry { n0: u8, n1: u8, n2: u8 },
    AddRegistry { x: u8, n0: u8, n1: u8 },

    SkipIfEqualXN { x: u8, n0: u8, n1: u8 },
    SkipIfNotEqualXN { x: u8, n0: u8, n1: u8 },
    SkipIfEqualXY { x: u8, y: u8 },
    SkipIfNotEqualXY { x: u8, y: u8 },
    Jump { n0: u8, n1: u8, n2: u8 },
    Subroutine { n0: u8, n1: u8, n2: u8 },

    Set { x: u8, y: u8 },
    Or { x: u8, y: u8 },
    And { x: u8, y: u8 },
    Xor { x: u8, y: u8 },
    Increment { x: u8, y: u8 },
    Decrement { x: u8, y: u8 },
    DecrementRev { x: u8, y: u8 },
    ShiftLeft { x: u8, y: u8 },
    ShiftRight { x: u8, y: u8 },

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
    stack: [u8; 8],
    sub_pointer: usize,
    i: u16,
    start: usize,
    end: usize,
    program_counter: usize,
    pixels: Vec<Point>,
}

impl Chip8 {
    fn new() -> Chip8 {
        Chip8 {
            data: [0; 4096],
            v: [0; 16],
            stack: [0; 8],
            sub_pointer: 0,
            i: 0,
            start: 512,
            end: 512,
            program_counter: 512,
            pixels: Vec::new(),
        }
    }

    fn load(&mut self, file_path: PathBuf) {
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
            if c11 == 0 {
                // 00E0
                Opcode::Clear
            } else {
                // 00EE
                Opcode::Return
            }
        } else if c00 == 6 {
            // 6XNN
            Opcode::NormalRegistry {
                x: c01,
                n0: c10,
                n1: c11,
            }
        } else if c00 == 10 {
            // ANNN
            Opcode::IndexRegistry {
                n0: c01,
                n1: c10,
                n2: c11,
            }
        } else if c00 == 7 {
            // 7XNN
            Opcode::AddRegistry {
                x: c01,
                n0: c10,
                n1: c11,
            }
        } else if c00 == 3 {
            // 3XNN
            Opcode::SkipIfEqualXN {
                x: c01,
                n0: c10,
                n1: c11,
            }
        } else if c00 == 4 {
            // 4XNN
            Opcode::SkipIfNotEqualXN {
                x: c01,
                n0: c10,
                n1: c11,
            }
        } else if c00 == 5 {
            // 5XY0
            Opcode::SkipIfEqualXY { x: c01, y: c10 }
        } else if c00 == 9 {
            // 9XY0
            Opcode::SkipIfNotEqualXY { x: c01, y: c10 }
        } else if c00 == 1 {
            // 1NNN
            Opcode::Jump {
                n0: c01,
                n1: c10,
                n2: c11,
            }
        } else if c00 == 2 {
            // 2NNN
            Opcode::Subroutine {
                n0: c01,
                n1: c10,
                n2: c11,
            }
        } else if c00 == 8 {
            if c11 == 0 {
                // 8XY0
                Opcode::Set { x: c01, y: c10 }
            } else if c11 == 1 {
                // 8XY1
                Opcode::Or { x: c01, y: c10 }
            } else if c11 == 2 {
                // 8XY2
                Opcode::And { x: c01, y: c10 }
            } else if c11 == 3 {
                // 8XY3
                Opcode::Xor { x: c01, y: c10 }
            } else if c11 == 4 {
                // 8XY4
                Opcode::Increment { x: c01, y: c10 }
            } else if c11 == 5 {
                // 8XY5
                Opcode::Decrement { x: c01, y: c10 }
            } else if c11 == 7 {
                // 8XY7
                Opcode::DecrementRev { x: c01, y: c10 }
            } else if c11 == 6 {
                // 8XY6
                Opcode::ShiftRight { x: c01, y: c10 }
            } else {
                // 8XYE
                Opcode::ShiftLeft { x: c01, y: c10 }
            }
        } else if c00 == 13 {
            // DXYN
            Opcode::Draw {
                x: c01,
                y: c10,
                n: c11,
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

    fn set_normal_registry(&mut self, x: u8, n0: u8, n1: u8) {
        self.v[x as usize] = Chip8::to_decimal(0, n0, n1) as u8;
    }

    fn set_index_registry(&mut self, n0: u8, n1: u8, n2: u8) {
        self.i = Chip8::to_decimal(n0, n1, n2);
    }

    fn add_registry(&mut self, x: u8, n0: u8, n1: u8) {
        self.v[x as usize] += Chip8::to_decimal(0, n0, n1) as u8;
    }

    fn skip_if_equal(&mut self, x: u8, n0: u8, n1: u8) {
        if self.v[x as usize] == self.v[Chip8::to_decimal(0, n0, n1) as usize] {
            self.step_counter();
        }
    }

    fn skip_if_not_equal(&mut self, x: u8, n0: u8, n1: u8) {
        if self.v[x as usize] != self.v[Chip8::to_decimal(0, n0, n1) as usize] {
            self.step_counter();
        }
    }

    fn jump(&mut self, n0: u8, n1: u8, n2: u8) {
        self.program_counter = Chip8::to_decimal(n0, n1, n2) as usize;
    }

    fn subroutine(&mut self, n0: u8, n1: u8, n2: u8) {
        self.jump(n0, n1, n2);
        self.stack[self.sub_pointer] = self.program_counter as u8;
        self.sub_pointer += 1;
    }

    fn return_sub(&mut self) {
        self.program_counter = self.stack[self.sub_pointer - 1] as usize;
        self.stack[self.sub_pointer - 1] = 0;
        self.sub_pointer -= 1;
    }

    fn set(&mut self, x: u8, y: u8) {
        self.v[x as usize] = self.v[y as usize];
    }

    fn or(&mut self, x: u8, y: u8) {
        self.v[x as usize] |= self.v[y as usize];
    }

    fn and(&mut self, x: u8, y: u8) {
        self.v[x as usize] &= self.v[y as usize];
    }

    fn xor(&mut self, x: u8, y: u8) {
        self.v[x as usize] ^= self.v[y as usize];
    }

    fn increment(&mut self, x: u8, y: u8) {
        self.v[x as usize] += self.v[y as usize];
    }

    fn decrement(&mut self, x: u8, y: u8) {
        self.v[x as usize] -= self.v[y as usize];
    }

    fn decrement_rev(&mut self, x: u8, y: u8) {
        self.v[y as usize] -= self.v[x as usize];
    }

    fn shift_left(&mut self, x: u8, y: u8) {
        self.v[15] = (self.v[x as usize] & 0b10000000) >> 7;
        self.v[x as usize] = self.v[y as usize] << 1;
    }

    fn shift_right(&mut self, x: u8, y: u8) {
        self.v[15] = self.v[x as usize] & 0b1;
        self.v[x as usize] = self.v[y as usize] >> 1;
    }

    fn draw(&mut self, x: u8, y: u8, n: u8) {
        let px = self.v[x as usize];
        let py = self.v[y as usize];

        for oy in 0..n {
            let idx = oy as usize + self.i as usize;
            let mut bit_row = self.data[idx];
            for ox in 0..8 {
                let bit = bit_row & 0b1;
                bit_row >>= 1;
                if bit > 0 {
                    self.draw_pixel(px + (8 - ox), py + oy);
                }
            }
        }
    }

    fn draw_pixel(&mut self, x: u8, y: u8) {
        let pixel = Point::new(x as i32, y as i32);
        self.pixels.push(pixel);
    }

    fn step_counter(&mut self) {
        self.program_counter += 2;
    }

    fn execute(&mut self) {
        let opcode = Chip8::decode(self.fetch());
        match opcode {
            Opcode::Clear => {
                self.pixels.clear();
                self.step_counter();
            }
            Opcode::Return => {
                self.return_sub();
            }
            Opcode::NormalRegistry { x, n0, n1 } => {
                self.set_normal_registry(x, n0, n1);
                self.step_counter();
            }
            Opcode::IndexRegistry { n0, n1, n2 } => {
                self.set_index_registry(n0, n1, n2);
                self.step_counter();
            }
            Opcode::AddRegistry { x, n0, n1 } => {
                self.add_registry(x, n0, n1);
                self.step_counter();
            }
            Opcode::SkipIfEqualXN { x, n0, n1 } => {
                self.skip_if_equal(x, n0, n1);
                self.step_counter();
            }
            Opcode::SkipIfNotEqualXN { x, n0, n1 } => {
                self.skip_if_not_equal(x, n0, n1);
                self.step_counter();
            }
            Opcode::SkipIfEqualXY { x, y } => {
                self.skip_if_equal(x, 0, y);
                self.step_counter();
            }
            Opcode::SkipIfNotEqualXY { x, y } => {
                self.skip_if_not_equal(x, 0, y);
                self.step_counter();
            }
            Opcode::Jump { n0, n1, n2 } => {
                self.jump(n0, n1, n2);
            }
            Opcode::Subroutine { n0, n1, n2 } => {
                self.subroutine(n0, n1, n2);
            }
            Opcode::Set { x, y } => {
                self.set(x, y);
                self.step_counter();
            }
            Opcode::Or { x, y } => {
                self.or(x, y);
                self.step_counter();
            }
            Opcode::And { x, y } => {
                self.and(x, y);
                self.step_counter();
            }
            Opcode::Xor { x, y } => {
                self.xor(x, y);
                self.step_counter();
            }
            Opcode::Increment { x, y } => {
                self.increment(x, y);
                self.step_counter();
            }
            Opcode::Decrement { x, y } => {
                self.decrement(x, y);
                self.step_counter();
            }
            Opcode::DecrementRev { x, y } => {
                self.decrement_rev(x, y);
                self.step_counter();
            }
            Opcode::ShiftRight { x, y } => {
                self.shift_right(x, y);
                self.step_counter();
            }
            Opcode::ShiftLeft { x, y } => {
                self.shift_left(x, y);
                self.step_counter();
            }
            Opcode::Draw { x, y, n } => {
                self.draw(x, y, n);
                self.step_counter();
            }
        }
    }

    fn display(&self, canvas: &mut Canvas<Window>) {
        canvas.set_draw_color(Color::RGB(0, 25, 0));
        canvas.clear();
        canvas.set_draw_color(Color::RGB(0, 175, 0));
        for pixel in self.pixels.iter() {
            canvas.draw_point(*pixel).unwrap();
        }
        canvas.present();
    }
}
