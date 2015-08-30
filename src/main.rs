extern crate portmidi as pm;
extern crate portaudio;
extern crate num;

use std::f64;
use std::thread::sleep_ms;
use std::sync::{Arc, RwLock, Mutex};
use std::thread;
use std::io::stdin;
use std::sync::mpsc::{Sender, Receiver, channel};
use std::error::Error;

use pm::PortMidiResult;
use pm::PortMidiError::InvalidDeviceId;
use portaudio::pa;

const SAMPLE_RATE: f64 = 44_100.0;
const FRAMES: usize = 256;
const DELTATIME: f64 = 1.0 / SAMPLE_RATE;

fn main() {
    if let Some(device_id) = get_device() {
        if let Err(e) = handle_device(device_id) {
            println!("Error: {:?}", e);
        }
    } else {
        println!("Error: No device found!");
    }
}

fn get_device() -> Option<i32> {
    let _ = pm::initialize();

    let count = pm::count_devices();
    let device = (0..count).filter_map(|i| pm::get_device_info(i))
        .filter(|i| i.input)
        .next()
        .and_then(|i| Some(i.device_id));

    let _ = pm::terminate();
    device
}

fn handle_device(id: i32) -> PortMidiResult<()> {
    try!(pm::get_device_info(id).ok_or(InvalidDeviceId));
    let mut input = pm::InputPort::new(id, 1024);
    try!(input.open());

    let quit_watcher = QuitWatcher::new();
    quit_watcher.start();
    let server = note_server();

    while quit_watcher.is_running() {
        while let Some(event) = try!(input.read()) {
            let key = event.message.data1;
            let velocity = event.message.data2;
            let note = match event.message.status {
                144 => if velocity == 0 {
                    Midi::KeyReleased(key)
                } else {
                    Midi::KeyPressed(key, velocity)
                },
                176 => Midi::Knob(key, velocity),
                192 => Midi::Button(key),
                224 => Midi::PitchBend(velocity),
                _   => Midi::Unknown(event.message.status, key, velocity)
            };
            let _ = server.send(note);
        }

        sleep_ms(50);
    }
    
    try!(input.close());
    Ok(())
}

#[derive(Debug)]
enum Midi {
    KeyPressed(u8, u8),
    KeyReleased(u8),
    Knob(u8, u8),
    Button(u8),
    PitchBend(u8),
    Unknown(u8, u8, u8),
}

fn note_server() -> Sender<Midi> {
    let (send, recv) = channel();
    let note = Arc::new(Mutex::new(Vec::new()));
    let send_note = note.clone();
    thread::spawn(move || notes(recv, note));
    thread::spawn(move || synth(send_note));
    send
}

fn notes(recv: Receiver<Midi>, notes: Arc<Mutex<Vec<(f64, f64)>>>) {
    fn pitch_from_key(key: u8) -> f64 {
        1.05946309436f64.powi(key as i32 - 49) * 440.0
    }

    loop {
        match recv.recv().unwrap() {
            Midi::KeyPressed(key, _) => {
                let mut guard = notes.lock().unwrap();
                let pitch = pitch_from_key(key);
                guard.push((pitch, 0.0));
            }
            Midi::KeyReleased(key) => {
                let mut guard = notes.lock().unwrap();
                let pitch = pitch_from_key(key);
                guard.retain(|&(x, _)| x != pitch);
            }
            _ => (),
        };
    }
}

fn synth(note: Arc<Mutex<Vec<(f64, f64)>>>) -> Result<(), pa::Error> {
    try!(pa::initialize());
    
    let dev_out = pa::device::get_default_output();
    let output_info = pa::device::get_info(dev_out).unwrap();
    let out_params = pa::StreamParameters {
        device: dev_out,
        channel_count: 1,
        sample_format: pa::SampleFormat::Float32,
        suggested_latency: output_info.default_low_output_latency
    };

    let callback = Box::new(move |
                            _input: &[f32],
                            output: &mut[f32],
                            frames: u32,
                            _time_info: &pa::StreamCallbackTimeInfo,
                            _flags: pa::StreamCallbackFlags
                            | -> pa::StreamCallbackResult {
                                assert!(frames == FRAMES as u32);
                                let mut guard = note.lock().unwrap();                                
                                for sample in output.iter_mut() {
                                    *sample = 0.0;
                                    for &mut (pitch, ref mut time) in guard.iter_mut() {
                                        *time += DELTATIME*pitch;
                                        *sample += ((*time*f64::consts::PI*2.0).sin() * 0.1) as f32;
                                    }
                                }
                                pa::StreamCallbackResult::Continue
                            });

    let mut stream: pa::Stream<f32, f32> = pa::Stream::new();
    let _ = stream.open(None, Some(&out_params), SAMPLE_RATE,
                        FRAMES as u32, pa::StreamFlags::empty(),
                        Some(callback));

    try!(stream.start());
    while let Ok(true) = stream.is_active() { thread::sleep_ms(100); }
    try!(stream.close());
    try!(pa::terminate());
    Ok(())
}

struct QuitWatcher(Arc<RwLock<bool>>);
impl QuitWatcher {
    fn new() -> QuitWatcher {
        QuitWatcher(Arc::new(RwLock::new(false)))
    }

    fn start(&self) {
        let quit_lock = self.0.clone();
        thread::spawn(move || {
            println!("Press enter to quit");
            stdin().read_line(&mut String::new()).ok().expect("Failed read line");
            let mut quit = quit_lock.write().unwrap();
            *quit = true;
        });
    }

    fn is_running(&self) -> bool {
        let quit = self.0.read().unwrap();
        !*quit
    }
}
