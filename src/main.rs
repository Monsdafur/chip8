use std::fs;
use std::io::Read;
use std::time::Duration;

use sdl3::{event::Event, pixels::Color, render::FRect};

fn sdl() {
    let sdl_context = sdl3::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem.window("chip8", 800, 600).build().unwrap();

    let mut canvas = window.into_canvas();
    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();
    canvas.present();
    let mut event_pump = sdl_context.event_pump().unwrap();
    let rect = FRect::new(380.0, 280.0, 40.0, 40.0);

    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    break 'running;
                }
                _ => {}
            }
        }
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        canvas.set_draw_color(Color::RGB(255, 255, 255));
        canvas.draw_rect(rect).unwrap();
        canvas.present();
        std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}

struct Memory {
    data: [u8; 4096],
}

fn main() {
    let mut data = Vec::new();
    let mut file = fs::File::open("data/1-chip8-logo.ch8").unwrap();
    let mut memory = Memory { data: [0; 4096] };

    file.read_to_end(&mut data).unwrap();
    memory.data[512..512 + data.len()].copy_from_slice(&data[..]);
    let start = 512;
    let end = start + data.len();
    for i in start..end {
        print!("{} ", memory.data[i]);
    }
}
