use clap::Parser;
use std::io::Read;
use std::time::Instant;
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

    let window = video_subsystem.window("chip8", 960, 480).build().unwrap();

    let mut canvas = window.into_canvas();
    let mut event_pump = sdl_context.event_pump().unwrap();
    canvas.set_scale(15.0, 15.0).unwrap();

    let instant = Instant::now();
    let mut time;
    let mut last_frame_time = 0.0f32;
    let frame_rate_inv = 1.0f32 / 60.0f32;

    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    break 'running;
                }
                _ => {}
            }

            chip8.input_handle(&event);
        }

        time = instant.elapsed().as_secs_f32();
        let allow_display = (time - last_frame_time) > frame_rate_inv;

        chip8.execute(allow_display);
        chip8.display(&mut canvas);

        if allow_display {
            last_frame_time = time;
        }
    }
}

#[derive(Parser)]
struct Cli {
    path: PathBuf,
}

enum Opcode {
    Clear,
    Return,

    NormalRegistry { x: u8, n0: u8, n1: u8 },
    IndexRegistry { n0: u8, n1: u8, n2: u8 },
    AddRegistry { x: u8, n0: u8, n1: u8 },

    SaveToMemory { x: u8 },
    LoadFromMemory { x: u8 },
    AddVxToI { x: u8 },
    SaveDigits { x: u8 },

    SetTimer { x: u8 },
    SaveTimer { x: u8 },

    SkipIfEqualXN { x: u8, n0: u8, n1: u8 },
    SkipIfNotEqualXN { x: u8, n0: u8, n1: u8 },
    SkipIfEqualXY { x: u8, y: u8 },
    SkipIfNotEqualXY { x: u8, y: u8 },
    Jump { n0: u8, n1: u8, n2: u8 },
    JumpOffset { n0: u8, n1: u8, n2: u8 },
    Subroutine { n0: u8, n1: u8, n2: u8 },

    Set { x: u8, y: u8 },
    Or { x: u8, y: u8 },
    And { x: u8, y: u8 },
    Xor { x: u8, y: u8 },
    Add { x: u8, y: u8 },
    Subtract { x: u8, y: u8 },
    SubtractRev { x: u8, y: u8 },
    ShiftLeft { x: u8, y: u8 },
    ShiftRight { x: u8, y: u8 },

    SkipIfKeyDown { x: u8 },
    SkipIfKeyUp { x: u8 },
    WaitKeyDown { x: u8 },

    Draw { x: u8, y: u8, n: u8 },

    None { raw: RawOpCode },
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
    memory: [u8; 4096],
    registry: [u8; 16],
    stack: [usize; 8],
    key: [bool; 16],
    sub_pointer: usize,
    i: usize,
    start: usize,
    end: usize,
    program_counter: usize,
    pixels: Vec<Point>,
    pixel_map: [[u8; 32]; 64],
    timer: u8,
}

impl Chip8 {
    fn new() -> Chip8 {
        Chip8 {
            memory: [0; 4096],
            registry: [0; 16],
            stack: [0; 8],
            key: [false; 16],
            sub_pointer: 0,
            i: 0,
            start: 512,
            end: 512,
            program_counter: 512,
            pixels: Vec::new(),
            pixel_map: [[0; 32]; 64],
            timer: 0,
        }
    }

    fn load(&mut self, file_path: PathBuf) {
        let mut data = Vec::new();
        let mut file = fs::File::open(file_path).unwrap();

        file.read_to_end(&mut data).unwrap();
        self.end = self.start + data.len();
        self.memory[self.start..self.end].copy_from_slice(&data[..]);
    }

    fn fetch(&self) -> RawOpCode {
        RawOpCode {
            v0: self.memory[self.program_counter],
            v1: self.memory[self.program_counter + 1],
        }
    }

    fn decode(raw_opcode: RawOpCode) -> Opcode {
        let hex = ((raw_opcode.v0 as i32) << 8) | raw_opcode.v1 as i32;
        let c0 = ((hex & 0xF000) >> 12) as u8;
        let c1 = ((hex & 0x0F00) >> 8) as u8;
        let c2 = ((hex & 0x00F0) >> 4) as u8;
        let c3 = (hex & 0x000F) as u8;

        match c0 {
            0x0 => match c3 {
                0x0 => Opcode::Clear, // 00E0

                0xE => Opcode::Return, // 00EE

                _ => Opcode::None { raw: raw_opcode },
            },

            0x6 => Opcode::NormalRegistry {
                x: c1,
                n0: c2,
                n1: c3,
            }, // 6xnn

            0xA => Opcode::IndexRegistry {
                n0: c1,
                n1: c2,
                n2: c3,
            }, // Annn

            0x7 => Opcode::AddRegistry {
                x: c1,
                n0: c2,
                n1: c3,
            }, // 7xnn

            0xF => match raw_opcode.v1 {
                0x55 => Opcode::SaveToMemory { x: c1 }, // Fx55

                0x65 => Opcode::LoadFromMemory { x: c1 }, // Fx65

                0x1E => Opcode::AddVxToI { x: c1 }, // Fx1E

                0x33 => Opcode::SaveDigits { x: c1 }, // Fx33

                0x15 => Opcode::SetTimer { x: c1 }, // Fx15

                0x07 => Opcode::SaveTimer { x: c1 }, // Fx07

                0x0A => Opcode::WaitKeyDown { x: c1 }, // Fx0A

                _ => Opcode::None { raw: raw_opcode },
            },

            0x3 => Opcode::SkipIfEqualXN {
                x: c1,
                n0: c2,
                n1: c3,
            }, // 3Xnn

            0x4 => Opcode::SkipIfNotEqualXN {
                x: c1,
                n0: c2,
                n1: c3,
            }, // 4Xnn

            0x5 => Opcode::SkipIfEqualXY { x: c1, y: c2 }, // 5xy0

            0x9 => Opcode::SkipIfNotEqualXY { x: c1, y: c2 }, // 9xy0

            0x1 => Opcode::Jump {
                n0: c1,
                n1: c2,
                n2: c3,
            }, // 1nnn

            0xB => Opcode::JumpOffset {
                n0: c1,
                n1: c2,
                n2: c3,
            }, // Bnnn

            0x2 => Opcode::Subroutine {
                n0: c1,
                n1: c2,
                n2: c3,
            }, // 2nnn

            0x8 => match c3 {
                0x0 => Opcode::Set { x: c1, y: c2 }, // 8xy0

                0x1 => Opcode::Or { x: c1, y: c2 }, // 8xy1

                0x2 => Opcode::And { x: c1, y: c2 }, // 8xy2

                0x3 => Opcode::Xor { x: c1, y: c2 }, // 8xy3

                0x4 => Opcode::Add { x: c1, y: c2 }, // 8xy4

                0x5 => Opcode::Subtract { x: c1, y: c2 }, // 8xy5

                0x7 => Opcode::SubtractRev { x: c1, y: c2 }, // 8xy7

                0x6 => Opcode::ShiftRight { x: c1, y: c2 }, // 8xy6

                0xE => Opcode::ShiftLeft { x: c1, y: c2 }, // 8xyE

                _ => Opcode::None { raw: raw_opcode },
            },
            0xE => match raw_opcode.v1 {
                0x9E => Opcode::SkipIfKeyDown { x: c1 }, // Ex9E

                0xA1 => Opcode::SkipIfKeyUp { x: c1 }, // ExA1

                _ => Opcode::None { raw: raw_opcode },
            },

            0xD => Opcode::Draw {
                x: c1,
                y: c2,
                n: c3,
            }, // DxyN

            _ => Opcode::None { raw: raw_opcode },
        }
    }

    fn to_decimal(n0: u8, n1: u8, n2: u8) -> u16 {
        n0 as u16 * 256 + n1 as u16 * 16 + n2 as u16
    }

    fn set_normal_registry(&mut self, x: u8, n0: u8, n1: u8) {
        self.registry[x as usize] = Chip8::to_decimal(0, n0, n1) as u8;
    }

    fn set_index_registry(&mut self, n0: u8, n1: u8, n2: u8) {
        self.i = Chip8::to_decimal(n0, n1, n2) as usize;
    }

    fn add_registry(&mut self, x: u8, n0: u8, n1: u8) {
        let result =
            (self.registry[x as usize] as u16 + Chip8::to_decimal(0, n0, n1) as u16) & 0xFF;
        self.registry[x as usize] = result as u8;
    }

    fn save_to_memory(&mut self, x: u8) {
        let d = x as usize + 1;
        let s = self.i & 0xFFF;
        let e = (self.i + d) & 0xFFF;
        self.memory[s..e].copy_from_slice(&self.registry[0..d]);
        self.i += d;
    }

    fn load_from_memory(&mut self, x: u8) {
        let d = x as usize + 1;
        let s = self.i & 0xFFF;
        let e = (self.i + d) & 0xFFF;
        self.registry[0..d].copy_from_slice(&self.memory[s..e]);
        self.i += d;
    }

    fn add_vx_to_i(&mut self, x: u8) {
        self.i += self.registry[x as usize] as usize;
    }

    fn set_timer(&mut self, x: u8) {
        self.timer = self.registry[x as usize];
    }

    fn save_timer(&mut self, x: u8) {
        self.registry[x as usize] = self.timer;
    }

    fn save_digits(&mut self, x: u8) {
        let ci = self.i & 0xFFF;
        self.memory[ci] = self.registry[x as usize] / 100;
        self.memory[ci + 1] = (self.registry[x as usize] / 10) % 10;
        self.memory[ci + 2] = self.registry[x as usize] % 10;
    }

    fn skip_if_equal_xn(&mut self, x: u8, n0: u8, n1: u8) {
        if self.registry[x as usize] == Chip8::to_decimal(0, n0, n1) as u8 {
            self.step_counter();
        }
    }

    fn skip_if_equal_xy(&mut self, x: u8, y: u8) {
        if self.registry[x as usize] == self.registry[y as usize] {
            self.step_counter();
        }
    }

    fn skip_if_not_equal_xn(&mut self, x: u8, n0: u8, n1: u8) {
        if self.registry[x as usize] != Chip8::to_decimal(0, n0, n1) as u8 {
            self.step_counter();
        }
    }

    fn skip_if_not_equal_xy(&mut self, x: u8, y: u8) {
        if self.registry[x as usize] != self.registry[y as usize] {
            self.step_counter();
        }
    }

    fn jump(&mut self, n0: u8, n1: u8, n2: u8) {
        self.program_counter = Chip8::to_decimal(n0, n1, n2) as usize;
    }

    fn jump_offset(&mut self, n0: u8, n1: u8, n2: u8) {
        self.program_counter = (Chip8::to_decimal(n0, n1, n2) + self.registry[0] as u16) as usize;
    }

    fn subroutine(&mut self, n0: u8, n1: u8, n2: u8) {
        self.stack[self.sub_pointer] = self.program_counter;
        self.sub_pointer += 1;
        self.jump(n0, n1, n2);
    }

    fn return_subroutine(&mut self) {
        self.program_counter = self.stack[self.sub_pointer - 1];
        self.stack[self.sub_pointer - 1] = 0;
        self.sub_pointer -= 1;
    }

    fn set(&mut self, x: u8, y: u8) {
        self.registry[x as usize] = self.registry[y as usize];
    }

    fn or(&mut self, x: u8, y: u8) {
        self.registry[x as usize] |= self.registry[y as usize];
        self.registry[15] = 0;
    }

    fn and(&mut self, x: u8, y: u8) {
        self.registry[x as usize] &= self.registry[y as usize];
        self.registry[15] = 0;
    }

    fn xor(&mut self, x: u8, y: u8) {
        self.registry[x as usize] ^= self.registry[y as usize];
        self.registry[15] = 0;
    }

    fn add(&mut self, x: u8, y: u8) {
        let n = self.registry[x as usize] as u16 + self.registry[y as usize] as u16;
        self.registry[x as usize] = (n & 0xFF) as u8;
        self.registry[15] = (n > 255) as u8;
    }

    fn subtract(&mut self, x: u8, y: u8) {
        let n = self.registry[x as usize] as i16 - self.registry[y as usize] as i16;
        self.registry[x as usize] = (n & 0xFF) as u8;
        self.registry[15] = (n >= 0) as u8;
    }

    fn subtract_rev(&mut self, x: u8, y: u8) {
        let n = self.registry[y as usize] as i16 - self.registry[x as usize] as i16;
        self.registry[x as usize] = (n & 0xFF) as u8;
        self.registry[15] = (n >= 0) as u8;
    }

    fn shift_left(&mut self, x: u8, y: u8) {
        let r = self.registry[y as usize];
        self.registry[x as usize] = (r << 1) & 0xFF;
        self.registry[15] = (r & 0b10000000) >> 7;
    }

    fn shift_right(&mut self, x: u8, y: u8) {
        let r = self.registry[y as usize];
        self.registry[x as usize] = (r >> 1) & 0xFF;
        self.registry[15] = r & 0b00000001;
    }

    fn skip_if_keydown(&mut self, x: u8) {
        if self.key[self.registry[x as usize] as usize] == true {
            self.step_counter();
        }
    }

    fn skip_if_keyup(&mut self, x: u8) {
        if self.key[self.registry[x as usize] as usize] == false {
            self.step_counter();
        }
    }

    fn wait_keydown(&mut self, x: u8) {
        for i in 0..16 {
            if self.key[i] {
                self.registry[x as usize] = i as u8;
                self.step_counter();
                break;
            }
        }
    }

    fn draw(&mut self, x: u8, y: u8, n: u8) {
        let px = self.registry[x as usize] % 64;
        let py = self.registry[y as usize] % 32;

        for oy in 0..n {
            let idx = oy as usize + self.i;
            let mut bit_row = self.memory[idx];
            for ox in (0..8).rev() {
                let pixel = bit_row & 0b1;
                bit_row >>= 1;

                let dx = (px + ox) as usize;
                let dy = (py + oy) as usize;

                if dx >= 64 || dy >= 32 {
                    continue;
                }

                if pixel == 1 {
                    self.registry[15] = self.pixel_map[dx][dy];
                }

                self.pixel_map[dx][dy] ^= pixel;
            }
        }
    }

    fn step_counter(&mut self) {
        self.program_counter += 2;
    }

    fn execute(&mut self, allow_display: bool) {
        let opcode = Chip8::decode(self.fetch());
        match opcode {
            Opcode::Clear => {
                if allow_display {
                    self.pixels.clear();
                    self.pixel_map = [[0; 32]; 64];
                    self.step_counter();
                }
            }
            Opcode::Return => {
                self.return_subroutine();
                self.step_counter();
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
            Opcode::SaveToMemory { x } => {
                self.save_to_memory(x);
                self.step_counter();
            }
            Opcode::LoadFromMemory { x } => {
                self.load_from_memory(x);
                self.step_counter();
            }
            Opcode::AddVxToI { x } => {
                self.add_vx_to_i(x);
                self.step_counter();
            }
            Opcode::SetTimer { x } => {
                self.set_timer(x);
                self.step_counter();
            }
            Opcode::SaveTimer { x } => {
                self.save_timer(x);
                self.step_counter();
            }
            Opcode::SaveDigits { x } => {
                self.save_digits(x);
                self.step_counter();
            }
            Opcode::SkipIfEqualXN { x, n0, n1 } => {
                self.skip_if_equal_xn(x, n0, n1);
                self.step_counter();
            }
            Opcode::SkipIfNotEqualXN { x, n0, n1 } => {
                self.skip_if_not_equal_xn(x, n0, n1);
                self.step_counter();
            }
            Opcode::SkipIfEqualXY { x, y } => {
                self.skip_if_equal_xy(x, y);
                self.step_counter();
            }
            Opcode::SkipIfNotEqualXY { x, y } => {
                self.skip_if_not_equal_xy(x, y);
                self.step_counter();
            }
            Opcode::Jump { n0, n1, n2 } => {
                self.jump(n0, n1, n2);
            }
            Opcode::JumpOffset { n0, n1, n2 } => {
                self.jump_offset(n0, n1, n2);
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
            Opcode::Add { x, y } => {
                self.add(x, y);
                self.step_counter();
            }
            Opcode::Subtract { x, y } => {
                self.subtract(x, y);
                self.step_counter();
            }
            Opcode::SubtractRev { x, y } => {
                self.subtract_rev(x, y);
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
            Opcode::SkipIfKeyDown { x } => {
                self.skip_if_keydown(x);
                self.step_counter();
            }
            Opcode::SkipIfKeyUp { x } => {
                self.skip_if_keyup(x);
                self.step_counter();
            }
            Opcode::WaitKeyDown { x } => {
                self.wait_keydown(x);
            }
            Opcode::Draw { x, y, n } => {
                if allow_display {
                    self.draw(x, y, n);
                    self.step_counter();
                }
            }
            Opcode::None { raw } => {
                unimplemented!("opcode {} not implemented", raw.as_string())
            }
        }

        if allow_display {
            self.timer -= if self.timer > 0 { 1 } else { 0 };
        }
    }

    fn input_handle(&mut self, event: &Event) {
        match event {
            Event::KeyDown { keycode, .. } => match keycode {
                Some(Keycode::_1) => self.key[0x1] = true,
                Some(Keycode::_2) => self.key[0x2] = true,
                Some(Keycode::_3) => self.key[0x3] = true,
                Some(Keycode::_4) => self.key[0xC] = true,

                Some(Keycode::Q) => self.key[0x4] = true,
                Some(Keycode::W) => self.key[0x5] = true,
                Some(Keycode::E) => self.key[0x6] = true,
                Some(Keycode::R) => self.key[0xD] = true,

                Some(Keycode::A) => self.key[0x7] = true,
                Some(Keycode::S) => self.key[0x8] = true,
                Some(Keycode::D) => self.key[0x9] = true,
                Some(Keycode::F) => self.key[0xE] = true,

                Some(Keycode::Z) => self.key[0xA] = true,
                Some(Keycode::X) => self.key[0x0] = true,
                Some(Keycode::C) => self.key[0xB] = true,
                Some(Keycode::V) => self.key[0xF] = true,

                _ => {}
            },

            Event::KeyUp { keycode, .. } => match keycode {
                Some(Keycode::_1) => self.key[0x1] = false,
                Some(Keycode::_2) => self.key[0x2] = false,
                Some(Keycode::_3) => self.key[0x3] = false,
                Some(Keycode::_4) => self.key[0xC] = false,

                Some(Keycode::Q) => self.key[0x4] = false,
                Some(Keycode::W) => self.key[0x5] = false,
                Some(Keycode::E) => self.key[0x6] = false,
                Some(Keycode::R) => self.key[0xD] = false,

                Some(Keycode::A) => self.key[0x7] = false,
                Some(Keycode::S) => self.key[0x8] = false,
                Some(Keycode::D) => self.key[0x9] = false,
                Some(Keycode::F) => self.key[0xE] = false,

                Some(Keycode::Z) => self.key[0xA] = false,
                Some(Keycode::X) => self.key[0x0] = false,
                Some(Keycode::C) => self.key[0xB] = false,
                Some(Keycode::V) => self.key[0xF] = false,

                _ => {}
            },

            _ => {}
        }
    }

    fn display(&self, canvas: &mut Canvas<Window>) {
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        canvas.set_draw_color(Color::RGB(0, 255, 0));

        let mut pixel = Point::new(0, 0);

        for x in 0..64usize {
            for y in 0..32usize {
                if self.pixel_map[x][y] == 1 {
                    pixel.x = x as i32;
                    pixel.y = y as i32;
                    canvas.draw_point(pixel).unwrap();
                }
            }
        }

        canvas.present();
    }
}
