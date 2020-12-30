extern crate btleplug;

use std::thread;
use std::time::Duration;

#[cfg(target_os = "linux")]
use btleplug::bluez::{adapter::ConnectedAdapter, manager::Manager};
#[cfg(target_os = "windows")]
use btleplug::winrtble::{adapter::Adapter, manager::Manager};
#[cfg(target_os = "macos")]
use btleplug::corebluetooth::{adapter::Adapter, manager::Manager};
use btleplug::api::{UUID, Central, Peripheral, NotificationHandler};

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn get_central(manager: &Manager) -> Adapter {
    let adapters = manager.adapters().unwrap();
    adapters.into_iter().nth(0).unwrap()
}

#[cfg(target_os = "linux")]
fn get_central(manager: &Manager) -> ConnectedAdapter {
    let adapters = manager.adapters().unwrap();
    let adapter = adapters.into_iter().nth(0).unwrap();
    adapter.connect().unwrap()
}


fn main() {
    let manager = Manager::new().unwrap();

    // get the first bluetooth adapter
    //
    // connect to the adapter
    let central = get_central(&manager);

    central.start_scan().unwrap();

    thread::sleep(Duration::from_secs(2));

    let ohr = central.peripherals().into_iter().find(|p| {
        p.properties().local_name.map(|n| n.starts_with("Polar OH1")).unwrap_or(false)
    }).unwrap();

    ohr.connect().expect("Couldn't connect");
    let mut bytes: [u8; 16] = [0x00,0x00,0x2A,0x37,0x00,0x00,0x10,0x00,0x80,0x00,0x00,0x80,0x5F,0x9B,0x34,0xFB];
    bytes.reverse();
    let uuid = UUID::B128(bytes);
    let chars = ohr.discover_characteristics().expect("Couldn't discover characteristics");
    let hr_char = chars.iter().find(|c| c.uuid == uuid).expect("couldn't find HR characteristic");

    let handler: NotificationHandler = Box::new(|not| {
        println!("{:?}", not.value);
    });
    ohr.on_notification(handler);
    ohr.subscribe(hr_char).expect("Couldn't subscribe");
    loop {

    }
}
