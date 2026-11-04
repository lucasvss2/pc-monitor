//! WinRT toasts with a COM activator for banner and Action Center activation.
//! Persisted launch arguments contain only the destination module, never session data.
use crate::agents::notifications::Sink;
use windows::{
    core::*,
    Data::Xml::Dom::XmlDocument,
    Win32::{Foundation::*, System::Com::*, UI::Notifications::*},
    UI::Notifications::*,
};

const APP_ID: &str = "dev.kvojps.monitor";
const ACTIVATOR: GUID = GUID::from_u128(0x57165877_a42d_4ff3_a2d9_b8b94ef32f09);
const CLSID: &str = "{57165877-A42D-4FF3-A2D9-B8B94EF32F09}";
type Activation = std::sync::Arc<dyn Fn() + Send + Sync>;

#[implement(INotificationActivationCallback)]
struct Activator(Activation);
impl INotificationActivationCallback_Impl for Activator_Impl {
    fn Activate(
        &self,
        app: &PCWSTR,
        args: &PCWSTR,
        _: *const NOTIFICATION_USER_INPUT_DATA,
        _: u32,
    ) -> Result<()> {
        // Reject unrelated activation; no arbitrary URL or command is accepted.
        if unsafe { app.to_string()? } == APP_ID && unsafe { args.to_string()? } == "agents" {
            (self.0)();
        }
        Ok(())
    }
}
#[implement(IClassFactory)]
struct Factory(Activation);
impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<IUnknown>,
        iid: *const GUID,
        out: *mut *mut std::ffi::c_void,
    ) -> Result<()> {
        if out.is_null() || iid.is_null() {
            return Err(E_POINTER.into());
        }
        unsafe {
            *out = std::ptr::null_mut();
        }
        if !outer.is_null() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let callback: INotificationActivationCallback = Activator(self.0.clone()).into();
        unsafe { callback.query(iid, out).ok() }
    }
    fn LockServer(&self, _: BOOL) -> Result<()> {
        Ok(())
    }
}

struct Registration(u32);
impl Drop for Registration {
    fn drop(&mut self) {
        unsafe {
            let _ = CoRevokeClassObject(self.0);
            CoUninitialize();
        }
    }
}

fn register(activate: Activation) -> Result<Registration> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
    }
    let registration = (|| {
        let user = &windows_registry::CURRENT_USER;
        let app = user.create(format!(r"Software\Classes\AppUserModelId\{APP_ID}"))?;
        app.set_string("DisplayName", "kvojps-monitor")?;
        app.set_string("CustomActivator", CLSID)?;
        let server = user.create(format!(r"Software\Classes\CLSID\{CLSID}\LocalServer32"))?;
        let executable = std::env::current_exe().map_err(|_| Error::from(E_FAIL))?;
        server.set_string("", format!("\"{}\" --toast-activate", executable.display()))?;
        let factory: IClassFactory = Factory(activate).into();
        unsafe {
            CoRegisterClassObject(
                &ACTIVATOR,
                &factory,
                CLSCTX_LOCAL_SERVER,
                REGCLS_MULTIPLEUSE,
            )
        }
        .map(Registration)
    })();
    if registration.is_err() {
        unsafe {
            CoUninitialize();
        }
    }
    registration
}

pub struct Desktop {
    _registration: Option<Registration>,
    notifier: Option<ToastNotifier>,
    next: u32,
}
impl Desktop {
    pub fn new(app: tauri::AppHandle) -> Self {
        let activate: Activation = std::sync::Arc::new(move || {
            let app = app.clone();
            let handle = app.clone();
            let _ =
                handle.run_on_main_thread(move || crate::panel::open_agent(&app, String::new()));
        });
        let registration = register(activate).ok();
        let notifier = registration
            .as_ref()
            .and_then(|_| ToastNotificationManager::CreateToastNotifierWithId(&APP_ID.into()).ok());
        Self {
            _registration: registration,
            notifier,
            next: 0,
        }
    }
}
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn payload(title: &str, body: &str) -> String {
    format!(
        r#"<toast launch="agents"><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual></toast>"#,
        escape(title),
        escape(body)
    )
}
impl Sink for Desktop {
    fn available(&self) -> bool {
        self.notifier.is_some()
    }
    fn show(&mut self, replaces: u32, title: &str, body: &str, _: &str) -> u32 {
        let Some(notifier) = &self.notifier else {
            return 0;
        };
        self.next = self.next.wrapping_add(1).max(1);
        let id = if replaces == 0 { self.next } else { replaces };
        let result = (|| -> Result<()> {
            let xml = XmlDocument::new()?;
            xml.LoadXml(&payload(title, body).into())?;
            let toast = ToastNotification::CreateToastNotification(&xml)?;
            toast.SetTag(&id.to_string().into())?;
            toast.SetGroup(&"agents".into())?;
            notifier.Show(&toast)
        })();
        if result.is_ok() {
            id
        } else {
            0
        }
    }
    fn close(&mut self, id: u32) {
        if id == 0 {
            return;
        }
        if let Ok(history) = ToastNotificationManager::History() {
            let _ = history.RemoveGroupedTagWithId(
                &id.to_string().into(),
                &"agents".into(),
                &APP_ID.into(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_com_class_dispatches_activation_without_session_payloads() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        let count = Arc::new(AtomicUsize::new(0));
        let calls = count.clone();
        let factory: IClassFactory = Factory(Arc::new(move || {
            calls.fetch_add(1, Ordering::SeqCst);
        }))
        .into();
        let class = GUID::from_u128(0xc4bca553_30d8_46b2_a7a2_65ea59c7e9f7);
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok().unwrap();
        }
        struct Cleanup(u32);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                unsafe {
                    let _ = CoRevokeClassObject(self.0);
                    CoUninitialize();
                }
            }
        }
        let cookie = unsafe {
            CoRegisterClassObject(&class, &factory, CLSCTX_LOCAL_SERVER, REGCLS_MULTIPLEUSE)
        }
        .unwrap();
        let _cleanup = Cleanup(cookie);
        let callback: INotificationActivationCallback =
            unsafe { CoCreateInstance(&class, None, CLSCTX_LOCAL_SERVER) }.unwrap();
        unsafe { callback.Activate(w!("dev.kvojps.monitor"), w!("agents"), &[]) }.unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn toast_payload_has_only_module_activation_and_escapes_xml() {
        let xml = payload("Codex", "Aguardando <ação> & aprovação");
        assert!(xml.contains(r#"launch="agents""#));
        assert!(xml.contains("&lt;ação&gt; &amp; aprovação"));
        assert!(!xml.contains("session"));
    }
    #[test]
    fn com_activator_validates_destination_before_calling_the_application() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        let count = Arc::new(AtomicUsize::new(0));
        let calls = count.clone();
        let callback: INotificationActivationCallback = Activator(Arc::new(move || {
            calls.fetch_add(1, Ordering::SeqCst);
        }))
        .into();
        unsafe {
            callback
                .Activate(w!("dev.kvojps.monitor"), w!("arbitrary-command"), &[])
                .unwrap();
            callback
                .Activate(w!("other.application"), w!("agents"), &[])
                .unwrap();
            assert_eq!(count.load(Ordering::SeqCst), 0);
            callback
                .Activate(w!("dev.kvojps.monitor"), w!("agents"), &[])
                .unwrap();
        }
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
}
