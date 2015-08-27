extern crate portmidi as pm;
use std::thread::sleep_ms;
use std::sync::{Arc, RwLock};
use std::thread;
use std::io::stdin;
use std::sync::mpsc::{Sender, Receiver, channel};

use pm::PortMidiResult;
use pm::PortMidiError::InvalidDeviceId;

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
    thread::spawn(move || {
        notes(recv)
    });
    send
}

fn notes(recv: Receiver<Midi>) {
    loop {
        let key = recv.recv().unwrap();
        println!("{:?}", key);
    }
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
