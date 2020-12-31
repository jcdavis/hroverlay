extern crate btleplug;
extern crate native_windows_gui as nwg;

use std::thread;
use std::time::Duration;

use btleplug::winrtble::manager::Manager;
use btleplug::api::{UUID, Central, Peripheral};
use nwg::NativeUi;
use std::env;
use std::sync::Arc;
use std::rc::Rc;
use std::sync::atomic::{AtomicU8, Ordering};

static HR_COUNT: AtomicU8 = AtomicU8::new(0);

#[derive(Default)]
pub struct BasicApp {
    window: nwg::Window,
    layout: nwg::GridLayout,
    hr: nwg::TextInput,
}

impl BasicApp {
    fn draw_hr(&self) {
        let value: u8 = HR_COUNT.load(Ordering::Relaxed);
        self.hr.set_text(value.to_string().as_str());
    }

    fn say_goodbye(&self) {
        nwg::stop_thread_dispatch();
    }

}

//
// ALL of this stuff is handled by native-windows-derive
//
mod basic_app_ui {
    use native_windows_gui as nwg;
    use super::*;
    use std::cell::RefCell;
    use std::ops::Deref;

    pub struct BasicAppUi {
        inner: Rc<BasicApp>,
        default_handler: RefCell<Option<nwg::EventHandler>>
    }

    impl nwg::NativeUi<BasicAppUi> for BasicApp {
        fn build_ui(mut data: BasicApp) -> Result<BasicAppUi, nwg::NwgError> {
            use nwg::Event as E;

            // Controls
            nwg::Window::builder()
                .flags(nwg::WindowFlags::WINDOW | nwg::WindowFlags::VISIBLE)
                .size((100, 50))
                .topmost(true)
                .title("Basic example")
                .build(&mut data.window)?;

            nwg::TextInput::builder()
                .text("--")
                .parent(&data.window)
                .readonly(true)
                .build(&mut data.hr)?;

            nwg::ControlBase::build_timer()
                .parent(Some(data.window.handle))
                .stopped(false)
                .interval(5)
                .build()?;


            // Wrap-up
            let ui = BasicAppUi {
                inner: Rc::new(data),
                default_handler: Default::default(),
            };

            // Events
            let evt_ui = Rc::downgrade(&ui.inner);
            let handle_events = move |evt, _evt_data, handle| {
                if let Some(evt_ui) = evt_ui.upgrade() {
                    match evt {
                        E::OnTimerTick => {
                                BasicApp::draw_hr(&evt_ui);
                            }
                        E::OnWindowClose =>
                            if &handle == &evt_ui.window {
                                BasicApp::say_goodbye(&evt_ui);
                            },
                        _ => {}
                    }
                }
            };

            *ui.default_handler.borrow_mut() = Some(nwg::full_bind_event_handler(&ui.window.handle, handle_events));

            // Layouts
            nwg::GridLayout::builder()
                .parent(&ui.window)
                .spacing(1)
                .child(0, 0, &ui.hr)
                .build(&ui.layout)?;

            return Ok(ui);
        }
    }

    impl Drop for BasicAppUi {
        /// To make sure that everything is freed without issues, the default handler must be unbound.
        fn drop(&mut self) {
            let handler = self.default_handler.borrow();
            if handler.is_some() {
                nwg::unbind_event_handler(handler.as_ref().unwrap());
            }
        }
    }

    impl Deref for BasicAppUi {
        type Target = BasicApp;

        fn deref(&self) -> &BasicApp {
            &self.inner
        }
    }
}

fn create_bt_updater() -> Result<(), &'static str> {
    let manager = Manager::new().map_err(|_e| "No manager")?;
    let adapters = manager.adapters().map_err(|_e| "Couldn't find adapter")?;
    let adapter = adapters.get(0).ok_or("Couldn't find adapter")?;

    adapter.start_scan().map_err(|_e|"couldn't scan BT")?;
    thread::sleep(Duration::from_secs(2));

    let ohr = adapter.peripherals().into_iter().find(|p| {
        p.properties().local_name.map(|n| n.starts_with("Polar OH1")).unwrap_or(false)
    }).ok_or("Couldn't find HRM")?;

    ohr.connect().map_err(|_e| "Couldn't connect")?;
    let mut bytes: [u8; 16] = [0x00,0x00,0x2A,0x37,0x00,0x00,0x10,0x00,0x80,0x00,0x00,0x80,0x5F,0x9B,0x34,0xFB];
    bytes.reverse();
    let uuid = UUID::B128(bytes);
    let chars = ohr.discover_characteristics().map_err(|_e| "Couldn't discover characteristics")?;
    let hr_char = chars.iter().find(|c| c.uuid == uuid).ok_or("couldn't find HR characteristic")?;

    ohr.on_notification(Box::new(|not| {
        println!("{:?}", not.value);
        HR_COUNT.store(not.value[1], Ordering::SeqCst);
    }));
    ohr.subscribe(hr_char).expect("Couldn't subscribe");
    Ok(())
}

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
    nwg::Font::set_global_family("Segoe UI").expect("Failed to set default font");
    let _ui = Arc::new(BasicApp::build_ui(Default::default()).expect("Failed to build UI"));

    nwg::dispatch_thread_events();
}
