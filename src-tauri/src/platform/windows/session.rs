//! Power callbacks feed the existing lifecycle port without owning a GUI window.
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use windows::Win32::{Foundation::HANDLE, System::Power::*, UI::WindowsAndMessaging::*};
type Queue = Arc<Mutex<VecDeque<bool>>>;
pub struct WindowsSession {
    registration: Option<HPOWERNOTIFY>,
    _context: Box<Queue>,
    queue: Queue,
}
// The registration is thread-independent; Windows only accesses the stable boxed queue.
unsafe impl Send for WindowsSession {}
unsafe extern "system" fn changed(
    context: *const std::ffi::c_void,
    event: u32,
    _: *const std::ffi::c_void,
) -> u32 {
    let sleeping = match event {
        PBT_APMSUSPEND => true,
        PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => false,
        _ => return 0,
    };
    let queue = unsafe { &*context.cast::<Queue>() };
    if let Ok(mut queue) = queue.lock() {
        queue.push_back(sleeping);
    }
    0
}
pub fn session() -> WindowsSession {
    let queue = Queue::default();
    let mut context = Box::new(queue.clone());
    let parameters = DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
        Callback: Some(changed),
        Context: (&mut *context as *mut Queue).cast(),
    };
    let mut registration = std::ptr::null_mut();
    let result = unsafe {
        PowerRegisterSuspendResumeNotification(
            DEVICE_NOTIFY_CALLBACK,
            HANDLE(
                (&parameters as *const DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS)
                    .cast_mut()
                    .cast(),
            ),
            &mut registration,
        )
    };
    WindowsSession {
        registration: (result.0 == 0).then_some(HPOWERNOTIFY(registration as isize)),
        _context: context,
        queue,
    }
}
impl crate::lifecycle::Session for WindowsSession {
    fn preparing_for_sleep(&self) -> Option<bool> {
        self.queue.lock().unwrap().pop_front()
    }
}
impl Drop for WindowsSession {
    fn drop(&mut self) {
        if let Some(registration) = self.registration.take() {
            unsafe {
                let _ = PowerUnregisterSuspendResumeNotification(registration);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle::Session;
    #[test]
    fn native_callback_preserves_suspend_resume_order() {
        let queue = Queue::default();
        let context = Box::new(queue.clone());
        let pointer = (&*context as *const Queue).cast();
        unsafe {
            changed(pointer, PBT_APMSUSPEND, std::ptr::null());
            changed(pointer, PBT_APMRESUMEAUTOMATIC, std::ptr::null());
        }
        let source = WindowsSession {
            registration: None,
            _context: context,
            queue,
        };
        assert_eq!(source.preparing_for_sleep(), Some(true));
        assert_eq!(source.preparing_for_sleep(), Some(false));
        assert_eq!(source.preparing_for_sleep(), None);
    }
}
