//! Trello REST adapter. Authentication stays here so a later move to OAuth 2.0
//! changes only this file.
use crate::ports::HttpError;
use serde_json::Value;

/// API key and token, borrowed for one request. Never logged, serialized or cached.
pub struct Auth<'a> {
    key: &'a str,
    token: &'a str,
}
impl<'a> Auth<'a> {
    pub fn new(key: &'a str, token: &'a str) -> Self {
        Self { key, token }
    }
    /// Header form keeps the token out of URLs.
    fn header(&self) -> String {
        format!(
            "OAuth oauth_consumer_key=\"{}\", oauth_token=\"{}\"",
            self.key, self.token
        )
    }
}

/// Token page for the user's own key: read and write, no expiration, shown in the
/// browser for pasting (no return_url capture).
pub fn authorization_url(key: &str) -> Option<String> {
    let key = key.trim();
    (!key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric())).then(|| {
        format!("https://trello.com/1/authorize?expiration=never&scope=read,write&response_type=token&name=kvojps-monitor&key={key}")
    })
}

/// Requests relative to https://api.trello.com/1, e.g. /members/me.
pub trait TrelloApi: Send {
    fn get(&self, auth: &Auth, path: &str) -> Result<Value, HttpError>;
    /// Update with query parameters; the response body is not needed.
    fn put(&self, auth: &Auth, path: &str) -> Result<(), HttpError>;
}

pub struct TrelloHttp;
impl TrelloHttp {
    fn send(
        method: reqwest::Method,
        auth: &Auth,
        path: &str,
    ) -> Result<reqwest::blocking::Response, HttpError> {
        let response = reqwest::blocking::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|_| HttpError::default())?
            .request(method, format!("https://api.trello.com/1{path}"))
            .header("Authorization", auth.header())
            .send()
            .map_err(|_| HttpError::default())?;
        if !response.status().is_success() {
            return Err(HttpError::response(
                response.status().as_u16(),
                response
                    .headers()
                    .get("retry-after")
                    .and_then(|value| value.to_str().ok()),
            ));
        }
        Ok(response)
    }
}
impl TrelloApi for TrelloHttp {
    fn get(&self, auth: &Auth, path: &str) -> Result<Value, HttpError> {
        let response = Self::send(reqwest::Method::GET, auth, path)?;
        let status = response.status().as_u16();
        response.json().map_err(|_| HttpError {
            status: Some(status),
            ..HttpError::default()
        })
    }
    fn put(&self, auth: &Auth, path: &str) -> Result<(), HttpError> {
        Self::send(reqwest::Method::PUT, auth, path).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authorization_url_requests_read_write_token_without_expiration_or_return_url() {
        let url = authorization_url(" 0123abcd ").unwrap();
        assert_eq!(url, "https://trello.com/1/authorize?expiration=never&scope=read,write&response_type=token&name=kvojps-monitor&key=0123abcd");
        assert!(!url.contains("return_url"));
        assert_eq!(authorization_url(""), None);
        assert_eq!(authorization_url("abc&return_url=x"), None);
    }
    #[test]
    fn credentials_travel_only_in_the_authorization_header() {
        assert_eq!(
            Auth::new("key", "token").header(),
            "OAuth oauth_consumer_key=\"key\", oauth_token=\"token\""
        );
    }
}
