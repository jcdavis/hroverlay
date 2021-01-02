extern crate btleplug;
extern crate native_windows_gui as nwg;

use std::thread;
use std::time::Duration;

use btleplug::winrtble::manager::Manager;
use btleplug::api::{UUID, Central, Peripheral};
use nwg::NativeUi;
use std::env;
use std::rc::Rc;
use std::sync::atomic::{AtomicU8, Ordering};

// The BTLE notification callback requires 'static for some reason :|
static HR_COUNT: AtomicU8 = AtomicU8::new(0);

#[derive(Default)]
pub struct HRViewer {
    window: nwg::Window,
    layout: nwg::GridLayout,
    font: nwg::Font,
    hr: nwg::TextInput,
}

impl HRViewer {
    fn draw_hr(&self) {
        match HR_COUNT.load(Ordering::SeqCst) {
            0 => {
                self.hr.set_text("--");
            }
            rest => {
                self.hr.set_text(rest.to_string().as_str());
            }
        }
    }

    fn exit(&self) {
        nwg::stop_thread_dispatch();
    }
}

/*
 * This mostly is copy-pasted from the basic layout example:
 * https://github.com/gabdube/native-windows-gui/blob/master/native-windows-gui/examples/basic_layout.rs
 * There are nice macros to do most of this for you, but we need to do custom hacks so not an option
 */
mod basic_app_ui {
    use native_windows_gui as nwg;
    use super::*;
    use std::cell::RefCell;
    use std::ops::Deref;
    use nwg::{ControlBase, NwgError, HTextAlign};
    use winapi::um::winuser::{WS_CLIPCHILDREN, WS_VISIBLE, WS_EX_TOPMOST, WS_EX_LAYERED, WS_POPUP, SetLayeredWindowAttributes, LWA_COLORKEY};
    use winapi::um::wingdi::RGB;

    pub struct HRViewerUi {
        inner: Rc<HRViewer>,
        default_handler: RefCell<Option<nwg::EventHandler>>
    }

    impl nwg::NativeUi<HRViewerUi> for HRViewer {
        fn build_ui(mut data: HRViewer) -> Result<HRViewerUi, nwg::NwgError> {
            use nwg::Event as E;

            // Controls
            data.window = Default::default();
            data.window.handle = ControlBase::build_hwnd()
                .class_name("NativeWindowsGuiWindow")
                .forced_flags(WS_CLIPCHILDREN)
                .ex_flags(WS_EX_TOPMOST | WS_EX_LAYERED)
                .flags(WS_POPUP | WS_VISIBLE)
                .size((100, 100))
                //TODO: figure out how to not hardcode this
                .position((1820, 1100))
                .build()?;

            nwg::Font::builder()
                .family("Arial")
                .size(50)
                .weight(700)
                .build(&mut data.font)?;

            nwg::TextInput::builder()
                .text("--")
                .size((100, 120))
                .parent(&data.window)
                .font(Some(&data.font))
                .align(HTextAlign::Right)
                .background_color(Some([255, 0, 0]))
                .readonly(true)
                .build(&mut data.hr)?;

            nwg::ControlBase::build_timer()
                .parent(Some(data.window.handle))
                .stopped(false)
                .interval(500)
                .build()?;


            // Wrap-up
            let ui = HRViewerUi {
                inner: Rc::new(data),
                default_handler: Default::default(),
            };

            // Events
            let evt_ui = Rc::downgrade(&ui.inner);
            let handle_events = move |evt, _evt_data, handle| {
                if let Some(evt_ui) = evt_ui.upgrade() {
                    match evt {
                        E::OnTimerTick => {
                                HRViewer::draw_hr(&evt_ui);
                            }
                        E::OnWindowClose =>
                            if &handle == &evt_ui.window {
                                HRViewer::exit(&evt_ui);
                            },
                        _ => {}
                    }
                }
            };

            *ui.default_handler.borrow_mut() = Some(nwg::full_bind_event_handler(&ui.window.handle, handle_events));

            // Layouts
            nwg::GridLayout::builder()
                .parent(&ui.window)
                .spacing(0)
                .margin([0, 0, 0, 0])
                .child(0, 0, &ui.hr)
                .build(&ui.layout)?;

            // We set the background color of the text box as red, mark as transparent
            match ui.window.handle {
                nwg::ControlHandle::Hwnd(hwnd) => {
                    unsafe {
                        SetLayeredWindowAttributes(hwnd, RGB(255, 0, 0), 0, LWA_COLORKEY);
                    }
                }
                _ => {
                    return Err(NwgError::InitializationError("??".to_string()));
                }
            }
            return Ok(ui);
        }
    }

    impl Drop for HRViewerUi {
        /// To make sure that everything is freed without issues, the default handler must be unbound.
        fn drop(&mut self) {
            let handler = self.default_handler.borrow();
            if handler.is_some() {
                nwg::unbind_event_handler(handler.as_ref().unwrap());
            }
        }
    }

    impl Deref for HRViewerUi {
        type Target = HRViewer;

        fn deref(&self) -> &HRViewer {
            &self.inner
        }
    }
}

// Very hacky - hardcodes the device type, doesn't handle disconnects etc
fn create_bt_updater() -> Result<(), &'static str> {
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

    ohr.on_notification(Box::new(|not| {
        let hr: u8 = if not.value[0] != 0 {
            0
        } else {
            not.value[1]
        };
        HR_COUNT.store(hr, Ordering::SeqCst);
    }));
    ohr.subscribe(hr_char).expect("Couldn't subscribe");
    Ok(())
}

// For basic UI testing. Just throws a bunch of data in the 0-199 range as a placeholder
fn create_dummy_updater() {
    thread::spawn(|| {
        let mut count: i64 = 0;
        loop {
            let r: u8 = (count % 200) as u8;
            HR_COUNT.store(r, Ordering::SeqCst);
            thread::sleep(Duration::from_millis(100));
            count += 1;
        }
    });
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        create_dummy_updater();
    } else {
        create_bt_updater().unwrap();
    }
    nwg::init().expect("Failed to init Native Windows GUI");
    let _ui = HRViewer::build_ui(Default::default()).expect("Failed to build UI");

    nwg::dispatch_thread_events();
}
