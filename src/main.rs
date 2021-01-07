extern crate btleplug;
extern crate native_windows_gui as nwg;
extern crate native_windows_derive as nwd;

use std::thread;
use std::time::Duration;

use btleplug::winrtble::manager::Manager;
use btleplug::api::{UUID, Central, Peripheral};
use nwd::NwgUi;
use nwg::NativeUi;
use winapi::um::winuser::{WS_EX_TOPMOST, WS_EX_LAYERED};
use std::env;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

#[derive(Default, NwgUi)]
pub struct HRViewer {
    #[nwg_control(size: (100, 100), position: (300, 300), flags: "POPUP|VISIBLE", ex_flags: WS_EX_TOPMOST|WS_EX_LAYERED)]
    #[nwg_events( OnInit: [HRViewer::setup_hr_thread], OnWindowClose: [HRViewer::close] )]
    window: nwg::Window,

    #[nwg_layout(parent: window, margin: [0,0,0,0], spacing: 0)]
    layout: nwg::GridLayout,

    #[nwg_resource(family: "Arial", size: 50, weight: 700)]
    font: nwg::Font,
    
    #[nwg_control(text: "--", size: (100, 120), font: Some(&data.font), h_align: HTextAlign::Right, background_color: Some([255, 0, 0]))]
    #[nwg_layout_item(layout: layout, row: 0, col: 0)]
    hr: nwg::Label,

    #[nwg_control]
    #[nwg_events(OnNotice: [HRViewer::draw_hr])]
    notice: nwg::Notice,

    hr_count: Arc<AtomicU8>,
}

impl HRViewer {

    fn setup_hr_thread(&self) {
        use winapi::um::winuser::{SetLayeredWindowAttributes, LWA_COLORKEY};
        use winapi::um::wingdi::RGB;

        let hr_notice = self.notice.sender();
        let args: Vec<String> = env::args().collect();
        if args.len() > 1 {
            create_dummy_updater(self, hr_notice);
        } else {
            create_bt_updater(self, hr_notice).unwrap();
        }

        match self.window.handle {
            nwg::ControlHandle::Hwnd(hwnd) => {
                unsafe {
                    SetLayeredWindowAttributes(hwnd, RGB(255, 0, 0), 0, LWA_COLORKEY);
                }
            }
            _ => {
                panic!("Bad handle type for window!")
            }
        }
    }

    fn draw_hr(&self) {
        match self.hr_count.load(Ordering::SeqCst) {
            0 => {
                self.hr.set_text("--");
            }
            rest => {
                self.hr.set_text(rest.to_string().as_str());
            }
        }
    }

    fn close(&self) {
        nwg::stop_thread_dispatch()
    }

}

// Very hacky - hardcodes the device type, doesn't handle disconnects etc
fn create_bt_updater(viewer: &HRViewer, notice: nwg::NoticeSender) -> Result<(), &'static str> {
    let manager = Manager::new().map_err(|_e| "No manager")?;
    let adapters = manager.adapters().map_err(|_e| "Couldn't find adapter")?;
    let adapter = adapters.get(0).ok_or("Couldn't find adapter")?;

    adapter.start_scan().map_err(|_e|"couldn't scan BT")?;
    thread::sleep(Duration::from_secs(2));

    let ohr = adapter.peripherals().into_iter().find(|p| {
        p.properties().local_name.map(|n| n.starts_with("Polar OH1")).unwrap_or(false)
    }).ok_or("Couldn't find HRM")?;

    adapter.stop_scan().map_err(|_e| "??")?;

    ohr.connect().map_err(|_e| "Couldn't connect")?;
    // 0x2A37 is the heart rate measurement characteristic
    // 00000000-0000-1000-8000-00805F9B34FB is the base 128 bit bluetooth UUID
    let mut bytes: [u8; 16] = [0x00,0x00,0x2A,0x37,0x00,0x00,0x10,0x00,0x80,0x00,0x00,0x80,0x5F,0x9B,0x34,0xFB];
    bytes.reverse();
    let uuid = UUID::B128(bytes);
    let chars = ohr.discover_characteristics().map_err(|_e| "Couldn't discover characteristics")?;
    let hr_char = chars.iter().find(|c| c.uuid == uuid).ok_or("couldn't find HR characteristic")?;

    let count_atomic = viewer.hr_count.clone();
    ohr.on_notification(Box::new(move |not| {
        let hr: u8 = if not.value[0] != 0 {
            0
        } else {
            not.value[1]
        };
        count_atomic.store(hr, Ordering::SeqCst);
        notice.notice();
    }));
    ohr.subscribe(hr_char).expect("Couldn't subscribe");
    Ok(())
}

// For basic UI testing. Just throws a bunch of data in the 0-199 range as a placeholder
fn create_dummy_updater(viewer: &HRViewer, notice: nwg::NoticeSender) {
    let count_atomic = viewer.hr_count.clone();
    thread::spawn(move || {
        let mut count: i64 = 0;
        loop {
            let r: u8 = (count % 200) as u8;
            count_atomic.store(r, Ordering::SeqCst);
            notice.notice();
            thread::sleep(Duration::from_millis(100));
            count += 1;
        }
    });
}

fn main() {
    nwg::init().expect("Failed to init Native Windows GUI");
    let _ui = HRViewer::build_ui(Default::default()).expect("Failed to build UI");
    nwg::dispatch_thread_events();
}
