extern crate btleplug;
extern crate native_windows_gui as nwg;

use std::thread;
use std::time::Duration;

use btleplug::winrtble::{adapter::Adapter, manager::Manager};
use btleplug::api::{UUID, Central, Peripheral, NotificationHandler, ValueNotification};
use crate::nwg::NativeUi;
use std::sync::Arc;
use std::rc::Rc;
use std::sync::atomic::{AtomicU8, Ordering};

static HR_COUNT: AtomicU8 = AtomicU8::new(0);

fn get_central(manager: &Manager) -> Adapter {
    let adapters = manager.adapters().unwrap();
    adapters.into_iter().nth(0).unwrap()
}

#[derive(Default)]
pub struct BasicApp {
    window: nwg::Window,
    layout: nwg::GridLayout,
    hr: nwg::TextInput,
    timer: nwg::Timer,
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
                .size((300, 115))
                .position((300, 300))
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
                .build();


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
                        /*E::OnButtonClick =>
                            if &handle == &evt_ui.hello_button {
                                BasicApp::say_hello(&evt_ui);
                            },*/
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

fn main() {
    let manager = Manager::new().unwrap();

    let central = get_central(&manager);

    central.start_scan().unwrap();

    thread::sleep(Duration::from_secs(2));

    let ohr = central.peripherals().into_iter().find(|p| {
        p.properties().local_name.map(|n| n.starts_with("Polar OH1")).unwrap_or(false)
    }).expect("Couldn't find the HRM");

    ohr.connect().expect("Couldn't connect");
    let mut bytes: [u8; 16] = [0x00,0x00,0x2A,0x37,0x00,0x00,0x10,0x00,0x80,0x00,0x00,0x80,0x5F,0x9B,0x34,0xFB];
    bytes.reverse();
    let uuid = UUID::B128(bytes);
    let chars = ohr.discover_characteristics().expect("Couldn't discover characteristics");
    let hr_char = chars.iter().find(|c| c.uuid == uuid).expect("couldn't find HR characteristic");

    nwg::init().expect("Failed to init Native Windows GUI");
    nwg::Font::set_global_family("Segoe UI").expect("Failed to set default font");
    let _ui = Arc::new(BasicApp::build_ui(Default::default()).expect("Failed to build UI"));

    let handler: Box<dyn FnMut(ValueNotification) + Send> = Box::new(|not| {
        println!("{:?}", not.value);
        HR_COUNT.store(not.value[1], Ordering::SeqCst);
    });
    ohr.on_notification(handler);
    ohr.subscribe(hr_char).expect("Couldn't subscribe");

    nwg::dispatch_thread_events();
    loop {

    }
}
