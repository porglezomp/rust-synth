extern crate portmidi as pm;
use std::thread::sleep_ms;
use std::sync::{Arc, RwLock};
use std::thread;
use std::io::stdin;

use pm::{PortMidiResult, DeviceInfo};
use pm::PortMidiError::InvalidDeviceId;

fn main() {
    let _ = pm::initialize();

    let count = pm::count_devices();
    let device = (0..count).filter_map(|i| pm::get_device_info(i))
        .filter(|i| i.input)
        .next();
    if let Some(DeviceInfo {device_id, ..}) = device {
        if let Err(e) = handle_device(device_id) {
            println!("Error: {:?}", e);
        }
    }

    let _ = pm::terminate();
}


fn handle_device(id: i32) -> PortMidiResult<()> {
    try!(pm::get_device_info(id).ok_or(InvalidDeviceId));
    let mut input = pm::InputPort::new(id, 1024);
    try!(input.open());

    let quit_watcher = QuitWatcher::new();
    quit_watcher.start();

    while quit_watcher.is_running() {
        while let Some(event) = try!(input.read()) {
            println!("{} {:?}", event.timestamp, event.message);
        }

        sleep_ms(50);
    }
    
    input.close()
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
