//! WinCred only: generic credentials bound to this user and machine, without roaming.
use crate::ports::Keyring;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::{Foundation::ERROR_NOT_FOUND, Security::Credentials::*};

pub struct NativeKeyring;

fn target(account: &str) -> Result<Vec<u16>, ()> {
    if account.is_empty() || account.contains('\0') || account.len() > 128 {
        return Err(());
    }
    Ok(format!("kvojps-monitor/{account}")
        .encode_utf16()
        .chain([0])
        .collect())
}

struct Credential(*mut CREDENTIALW);
impl Drop for Credential {
    fn drop(&mut self) {
        // WinCred owns the allocation, including its blob.
        unsafe { CredFree(self.0.cast()) }
    }
}
fn read(account: &str) -> Result<Option<Credential>, ()> {
    let target = target(account)?;
    let mut pointer = std::ptr::null_mut();
    match unsafe {
        CredReadW(
            PCWSTR(target.as_ptr()),
            CRED_TYPE_GENERIC,
            None,
            &mut pointer,
        )
    } {
        Ok(()) => Ok(Some(Credential(pointer))),
        Err(error) if error.code() == ERROR_NOT_FOUND.to_hresult() => Ok(None),
        Err(_) => Err(()),
    }
}

impl Keyring for NativeKeyring {
    fn read(&self, account: &str) -> Result<Option<String>, ()> {
        let Some(credential) = read(account)? else {
            return Ok(None);
        };
        let value = unsafe { &*credential.0 };
        if value.Type != CRED_TYPE_GENERIC || value.Persist != CRED_PERSIST_LOCAL_MACHINE {
            return Err(());
        }
        let length = value.CredentialBlobSize as usize;
        if length > CRED_MAX_CREDENTIAL_BLOB_SIZE as usize
            || (length > 0 && value.CredentialBlob.is_null())
        {
            return Err(());
        }
        let bytes = if length == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(value.CredentialBlob, length) }
        };
        String::from_utf8(bytes.to_vec()).map(Some).map_err(|_| ())
    }
    fn save(&self, account: &str, secret: &str) -> Result<(), ()> {
        if secret.len() > CRED_MAX_CREDENTIAL_BLOB_SIZE as usize {
            return Err(());
        }
        let mut target = target(account)?;
        let value = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target.as_mut_ptr()),
            CredentialBlobSize: secret.len() as u32,
            CredentialBlob: secret.as_ptr().cast_mut(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            ..Default::default()
        };
        unsafe { CredWriteW(&value, 0) }.map_err(|_| ())
    }
    fn delete(&self, account: &str) -> Result<(), ()> {
        let target = target(account)?;
        match unsafe { CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None) } {
            Ok(()) => Ok(()),
            Err(error) if error.code() == ERROR_NOT_FOUND.to_hresult() => Ok(()),
            Err(_) => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_inputs_are_rejected_before_accessing_the_vault() {
        assert!(NativeKeyring.save("bad\0account", "synthetic").is_err());
        assert!(NativeKeyring
            .save(
                "synthetic",
                &"x".repeat(CRED_MAX_CREDENTIAL_BLOB_SIZE as usize + 1)
            )
            .is_err());
    }
    #[test]
    #[ignore = "writes and removes an isolated synthetic credential in the real user vault"]
    fn native_vault_roundtrip_is_local_machine_and_cleanup_is_idempotent() {
        let account = format!(
            "native-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = NativeKeyring.delete(&self.0);
            }
        }
        let _cleanup = Cleanup(account.clone());
        assert!(NativeKeyring.read(&account).unwrap().is_none());
        NativeKeyring.save(&account, "SYNTHETIC_SECRET_á").unwrap();
        let raw = read(&account).unwrap().unwrap();
        assert_eq!(unsafe { (*raw.0).Persist }, CRED_PERSIST_LOCAL_MACHINE);
        assert_eq!(unsafe { (*raw.0).Type }, CRED_TYPE_GENERIC);
        drop(raw);
        // Do not print the credential even when the comparison fails.
        assert!(NativeKeyring.read(&account).unwrap().as_deref() == Some("SYNTHETIC_SECRET_á"));
        NativeKeyring.delete(&account).unwrap();
        NativeKeyring.delete(&account).unwrap();
        assert!(NativeKeyring.read(&account).unwrap().is_none());
    }
}
