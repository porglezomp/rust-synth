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
            let note = Note {
                key: event.message.data1,
                vel: event.message.data2
            };
            let _ = server.send(note);
        }

        sleep_ms(50);
    }
    
    try!(input.close());
    Ok(())
}

#[derive(Debug)]
struct Note { key: u8, vel: u8 }

fn note_server() -> Sender<Note> {
    let (send, recv) = channel();
    thread::spawn(move || {
        notes(recv)
    });
    send
}

fn notes(recv: Receiver<Note>) {
    loop {
        if let Ok(note) = recv.recv() {
            println!("{} {}", note.key, note.vel);
        }
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
