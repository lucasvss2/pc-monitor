//! Untrusted Windows protocol arguments contain only a fixed module destination.
pub const URI: &str = "kvojps-monitor://agents";
pub const FLAG: &str = "--notification-activate";

pub fn request(args: &[String]) -> Result<bool, ()> {
    if !args.iter().skip(1).any(|arg| arg == FLAG) {
        return Ok(false);
    }
    match args {
        [_, flag, uri]
            if flag == FLAG
                && (uri.eq_ignore_ascii_case(URI)
                    || uri.eq_ignore_ascii_case(&format!("{URI}/"))) =>
        {
            Ok(true)
        }
        _ => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).into()).collect()
    }
    #[test]
    fn accepts_only_the_fixed_module_without_private_payloads() {
        for uri in [URI, "KVOJPS-MONITOR://AGENTS/"] {
            assert_eq!(request(&args(&["monitor", FLAG, uri])), Ok(true));
        }
        assert_eq!(request(&args(&["monitor", "--autostart"])), Ok(false));
        for uri in [
            "kvojps-monitor://agents?session=PRIVATE_SESSION",
            "kvojps-monitor://agents#PRIVATE_PROJECT",
            "kvojps-monitor://other",
            "kvojps-monitor://user@agents",
            "kvojps-monitor://agents/extra",
            "kvojps-monitor://%61gents",
            "https://agents",
            "kvojps-monitor://agents\" --autostart",
        ] {
            assert_eq!(request(&args(&["monitor", FLAG, uri])), Err(()));
        }
        assert_eq!(request(&args(&["monitor", FLAG])), Err(()));
        assert_eq!(request(&args(&["monitor", FLAG, URI, "extra"])), Err(()));
        assert_eq!(request(&args(&["monitor", "extra", FLAG, URI])), Err(()));
    }
}
