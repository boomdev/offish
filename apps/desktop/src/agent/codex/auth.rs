//! Native Sign in with ChatGPT public client and Linux Secret Service storage.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{
    rand::{SecureRandom, SystemRandom},
    signature,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    fs::{DirBuilder, File, OpenOptions},
    io::{self, Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use url::{Url, form_urlencoded};

const ISSUER: &str = "https://auth.openai.com";
const AUTHORIZE: &str = "https://auth.openai.com/api/accounts/authorize";
const TOKEN: &str = "https://auth.openai.com/api/accounts/oauth/token";
const JWKS: &str = "https://auth.openai.com/.well-known/jwks.json";
const RESOURCE: &str = "https://api.openai.com/v1";
const DYNAMIC_CLIENT: &str = "dynamic_agent_client";
const CALLBACK: &str = "/auth/callback";
const SCOPES: &str =
    "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";
const PLAN_SCOPES: [&str; 2] = ["resource.invoke", "chatgpt.tokens.use.direct"];
const MAX_RESPONSE: usize = 256 * 1024;

#[derive(Clone)]
pub struct AccountStore(PathBuf);

#[derive(Serialize, Deserialize)]
struct Account {
    client: String,
    subject: String,
    token: String,
    refresh: String,
    expires_at: u64,
    scope: String,
    id_token: String,
}

#[derive(Deserialize)]
struct Tokens {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    token_type: String,
    expires_in: u64,
}

struct Attempt {
    state: String,
    nonce: String,
    verifier: String,
    redirect: String,
    client: Option<String>,
}

#[derive(PartialEq, Eq)]
struct Code {
    client: String,
    value: String,
}

impl AccountStore {
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }

    pub fn connected(&self) -> io::Result<bool> {
        let _lock = self.lock()?;
        Ok(load::<Account>("account")?.is_some())
    }

    pub fn disconnect(&self) -> io::Result<()> {
        let _lock = self.lock()?;
        if lookup("account")?.is_some() {
            service("clear", "account", None)?;
        }
        Ok(())
    }

    pub fn connect(&self) -> io::Result<()> {
        let _lock = self.lock()?;
        let previous: Option<Account> = load("account")?;
        let host = match load::<String>("host")? {
            Some(host) => host,
            None => {
                let mut bytes = [0; 16];
                SystemRandom::new()
                    .fill(&mut bytes)
                    .map_err(|_| auth_error())?;
                bytes[6] = (bytes[6] & 0x0f) | 0x40;
                bytes[8] = (bytes[8] & 0x3f) | 0x80;
                let hex = bytes
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                let host = format!(
                    "urn:uuid:{}-{}-{}-{}-{}",
                    &hex[..8],
                    &hex[8..12],
                    &hex[12..16],
                    &hex[16..20],
                    &hex[20..]
                );
                save("host", &host)?;
                host
            }
        };
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|_| auth_error())?;
        listener.set_nonblocking(true).map_err(|_| auth_error())?;
        let attempt = Attempt {
            state: random()?,
            nonce: random()?,
            verifier: random()?,
            redirect: format!(
                "http://127.0.0.1:{}{CALLBACK}",
                listener.local_addr()?.port()
            ),
            client: previous.as_ref().map(|account| account.client.clone()),
        };
        let url = attempt.url(&host);
        browser(url.as_str())?;
        let grant = receive(&listener, &attempt)?;
        let tokens: Tokens = request(
            TOKEN,
            Some(&[
                ("grant_type", "authorization_code"),
                ("client_id", &grant.client),
                ("code", &grant.value),
                ("code_verifier", &attempt.verifier),
                ("redirect_uri", &attempt.redirect),
                ("resource", RESOURCE),
            ]),
        )?;
        let keys: Keys = request(JWKS, None)?;
        let subject = verify_identity(
            tokens.id_token.as_deref().ok_or_else(auth_error)?,
            &keys,
            &grant.client,
            &attempt.nonce,
        )?;
        if previous.as_ref().is_some_and(|old| old.subject != subject) {
            return Err(io::Error::other(
                "Choose the connected ChatGPT account or disconnect it first.",
            ));
        }
        let account = account(tokens, grant.client, subject, None)?;
        save("account", &account)
    }

    pub(super) fn access_token(&self) -> io::Result<String> {
        let _lock = self.lock()?;
        let saved: Account = load("account")?
            .ok_or_else(|| io::Error::other("Connect your ChatGPT account first."))?;
        if saved.expires_at > now().saturating_add(60) {
            return Ok(saved.token);
        }
        let tokens: Tokens = request(
            TOKEN,
            Some(&[
                ("grant_type", "refresh_token"),
                ("client_id", &saved.client),
                ("refresh_token", &saved.refresh),
                ("resource", RESOURCE),
            ]),
        )
        .map_err(|_| io::Error::other("Could not renew ChatGPT access. Try connecting again."))?;
        let refreshed = account(
            tokens,
            saved.client.clone(),
            saved.subject.clone(),
            Some(saved),
        )?;
        save("account", &refreshed)?;
        Ok(refreshed.token)
    }

    fn lock(&self) -> io::Result<File> {
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&self.0)
            .map_err(|_| auth_error())?;
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY)
            .open(&self.0)
            .map_err(|_| auth_error())?;
        let metadata = directory.metadata().map_err(|_| auth_error())?;
        if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o077 != 0 {
            return Err(auth_error());
        }
        directory
            .try_lock()
            .map_err(|_| io::Error::other("Another ChatGPT account operation is in progress."))?;
        Ok(directory)
    }
}

impl Attempt {
    fn url(&self, host: &str) -> Url {
        let mut url = Url::parse(AUTHORIZE).expect("fixed authorization URL");
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(self.verifier.as_bytes()));
        let mut query = url.query_pairs_mut();
        query.extend_pairs([
            ("response_type", "code"),
            (
                "client_id",
                self.client.as_deref().unwrap_or(DYNAMIC_CLIENT),
            ),
            ("redirect_uri", &self.redirect),
            ("scope", SCOPES),
            ("resource", RESOURCE),
            ("ext_agent_host_id", host),
            ("state", &self.state),
            ("nonce", &self.nonce),
            ("code_challenge_method", "S256"),
            ("code_challenge", &challenge),
        ]);
        if self.client.is_none() {
            query.append_pair("agent_name_hint", "Offish");
        }
        drop(query);
        url
    }

    fn callback(&self, target: &str) -> Option<io::Result<Code>> {
        if !target.starts_with("/auth/callback?") {
            return None;
        }
        let mut values = std::collections::HashMap::new();
        for (key, value) in form_urlencoded::parse(target.split_once('?')?.1.as_bytes()) {
            if values.insert(key, value).is_some() {
                return None;
            }
        }
        if values.get("state")?.as_ref() != self.state {
            return None;
        }
        if values.contains_key("error") {
            return Some(Err(io::Error::other(
                "ChatGPT sign-in was declined or did not finish.",
            )));
        }
        let callback_id = values.get("client_id").map(|id| id.as_ref());
        let issued = match (&self.client, callback_id) {
            (Some(current), Some(other)) if current != other => return Some(Err(auth_error())),
            (Some(current), _) => current.as_str(),
            (None, Some(new))
                if new.starts_with("oaiapp_")
                    && new.len() <= 256
                    && new
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_') =>
            {
                new
            }
            _ => return Some(Err(auth_error())),
        };
        let Some(code) = values
            .get("code")
            .filter(|code| !code.is_empty() && code.len() <= 4096)
        else {
            return Some(Err(auth_error()));
        };
        Some(Ok(Code {
            client: issued.into(),
            value: code.to_string(),
        }))
    }
}

fn receive(listener: &TcpListener, attempt: &Attempt) -> io::Result<Code> {
    let until = Instant::now() + Duration::from_secs(180);
    while Instant::now() < until {
        match listener.accept() {
            Ok((mut socket, _)) => {
                let result = read_callback(&mut socket, attempt);
                let (code, message) = match &result {
                    Some(Ok(_)) => ("200 OK", "Sign-in response received. Return to Offish."),
                    _ => (
                        "400 Bad Request",
                        "Sign-in could not be completed. Return to Offish.",
                    ),
                };
                let _ = write!(
                    socket,
                    "HTTP/1.1 {code}\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{message}",
                    message.len()
                );
                if let Some(result) = result {
                    return result;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(40))
            }
            Err(_) => return Err(auth_error()),
        }
    }
    Err(io::Error::other("ChatGPT sign-in timed out."))
}

fn read_callback(socket: &mut TcpStream, attempt: &Attempt) -> Option<io::Result<Code>> {
    socket.set_nonblocking(true).ok()?;
    let until = Instant::now() + Duration::from_millis(500);
    let mut bytes = Vec::new();
    let mut chunk = [0; 1024];
    while Instant::now() < until && bytes.len() + chunk.len() <= 8192 {
        match socket.read(&mut chunk) {
            Ok(0) => return None,
            Ok(count) => bytes.extend_from_slice(&chunk[..count]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => return None,
        }
        if let Some(end) = bytes.iter().position(|byte| *byte == b'\n') {
            let mut request = std::str::from_utf8(&bytes[..end]).ok()?.split_whitespace();
            return (request.next()? == "GET")
                .then(|| attempt.callback(request.next()?))
                .flatten();
        }
    }
    None
}

fn browser(url: &str) -> io::Result<()> {
    let mut child = Command::new("/usr/bin/xdg-open")
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| io::Error::other("Could not open the system browser."))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

fn account(
    tokens: Tokens,
    client: String,
    subject: String,
    previous: Option<Account>,
) -> io::Result<Account> {
    let scope = tokens
        .scope
        .or_else(|| previous.as_ref().map(|old| old.scope.clone()))
        .unwrap_or_default();
    if !PLAN_SCOPES
        .iter()
        .all(|required| scope.split_whitespace().any(|scope| scope == *required))
    {
        return Err(io::Error::other("ChatGPT plan usage was not granted."));
    }
    if !tokens.token_type.eq_ignore_ascii_case("Bearer")
        || !valid_token(&tokens.access_token)
        || tokens.expires_in == 0
    {
        return Err(auth_error());
    }
    let id_token = previous
        .as_ref()
        .map(|old| old.id_token.clone())
        .or(tokens.id_token)
        .unwrap_or_default();
    let refresh = tokens
        .refresh_token
        .or_else(|| previous.map(|old| old.refresh))
        .filter(|value| valid_token(value))
        .ok_or_else(auth_error)?;
    Ok(Account {
        client,
        subject,
        token: tokens.access_token,
        refresh,
        expires_at: now().saturating_add(tokens.expires_in),
        scope,
        id_token,
    })
}

fn valid_token(value: &str) -> bool {
    !value.is_empty() && value.len() <= 32768 && value.bytes().all(|byte| byte.is_ascii_graphic())
}

#[derive(Deserialize)]
struct Keys {
    keys: Vec<Jwk>,
}
#[derive(Deserialize)]
struct Jwk {
    kid: String,
    kty: String,
    n: Option<String>,
    e: Option<String>,
}
#[derive(Deserialize)]
struct Header {
    kid: String,
    alg: String,
}
#[derive(Deserialize)]
struct Claims {
    iss: String,
    aud: Audience,
    sub: String,
    nonce: String,
    exp: u64,
    azp: Option<String>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Audience {
    Single(String),
    List(Vec<String>),
}

fn verify_identity(token: &str, keys: &Keys, client: &str, nonce: &str) -> io::Result<String> {
    let (signed, sig) = token.rsplit_once('.').ok_or_else(auth_error)?;
    let (head, body) = signed.split_once('.').ok_or_else(auth_error)?;
    let header: Header = decode(head)?;
    if header.alg != "RS256" {
        return Err(auth_error());
    }
    let key = keys
        .keys
        .iter()
        .find(|key| key.kid == header.kid && key.kty == "RSA")
        .ok_or_else(auth_error)?;
    signature::RsaPublicKeyComponents {
        n: bytes(key.n.as_deref().ok_or_else(auth_error)?)?,
        e: bytes(key.e.as_deref().ok_or_else(auth_error)?)?,
    }
    .verify(
        &signature::RSA_PKCS1_2048_8192_SHA256,
        signed.as_bytes(),
        &bytes(sig)?,
    )
    .map_err(|_| auth_error())?;
    let claims: Claims = decode(body)?;
    let audience = match claims.aud {
        Audience::Single(value) => value == client,
        Audience::List(values) => {
            values.iter().any(|value| value == client)
                && (values.len() == 1 || claims.azp.as_deref() == Some(client))
        }
    };
    if claims.iss != ISSUER
        || !audience
        || claims.nonce != nonce
        || claims.exp <= now()
        || claims.sub.trim().is_empty()
        || claims.azp.is_some_and(|party| party != client)
    {
        return Err(auth_error());
    }
    Ok(claims.sub)
}

fn random() -> io::Result<String> {
    let mut value = [0; 32];
    SystemRandom::new()
        .fill(&mut value)
        .map_err(|_| auth_error())?;
    Ok(URL_SAFE_NO_PAD.encode(value))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn bytes(value: &str) -> io::Result<Vec<u8>> {
    URL_SAFE_NO_PAD.decode(value).map_err(|_| auth_error())
}

fn decode<T: DeserializeOwned>(value: &str) -> io::Result<T> {
    serde_json::from_slice(&bytes(value)?).map_err(|_| auth_error())
}

fn request<T: DeserializeOwned>(endpoint: &str, fields: Option<&[(&str, &str)]>) -> io::Result<T> {
    let mut curl = Command::new("/usr/bin/curl");
    curl.env_clear().args([
        "-q",
        "--proto",
        "=https",
        "--fail",
        "--silent",
        "--max-time",
        "20",
        "--max-filesize",
        "262144",
    ]);
    let form = fields.map(|fields| {
        form_urlencoded::Serializer::new(String::new())
            .extend_pairs(fields.iter().copied())
            .finish()
    });
    if form.is_some() {
        curl.args([
            "--header",
            "Content-Type: application/x-www-form-urlencoded",
            "--data-binary",
            "@-",
        ]);
    }
    let (code, output) =
        run(curl.arg(endpoint), form.as_deref().map(str::as_bytes)).map_err(|_| auth_error())?;
    if code != 0 {
        return Err(auth_error());
    }
    serde_json::from_slice(&output).map_err(|_| auth_error())
}

fn lookup(entry: &str) -> io::Result<Option<Vec<u8>>> {
    service("lookup", entry, None)
}

fn load<T: DeserializeOwned>(entry: &str) -> io::Result<Option<T>> {
    lookup(entry)?
        .map(|value| serde_json::from_slice(&value).map_err(|_| auth_error()))
        .transpose()
}

fn save(entry: &str, value: &impl Serialize) -> io::Result<()> {
    let json = serde_json::to_vec(value).map_err(|_| auth_error())?;
    service("store", entry, Some(&json)).map(|_| ())
}

fn service(action: &str, entry: &str, input: Option<&[u8]>) -> io::Result<Option<Vec<u8>>> {
    let mut command = Command::new("/usr/bin/timeout");
    command.args(["15", "/usr/bin/secret-tool", action]);
    if action == "store" {
        command.arg("--label=Offish ChatGPT");
    }
    command.args(["application", "org.offish.desktop", "entry", entry]);
    let (state, bytes) = run(&mut command, input).map_err(|_| auth_error())?;
    match (action, state) {
        (_, 0) => Ok(Some(bytes)),
        ("lookup", 1) if bytes.is_empty() => Ok(None),
        _ => Err(io::Error::other(
            "The Linux Secret Service could not complete the operation.",
        )),
    }
}

fn run(command: &mut Command, input: Option<&[u8]>) -> io::Result<(i32, Vec<u8>)> {
    let mut child = command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let collected = (|| {
        if let Some(input) = input {
            child.stdin.take().expect("piped stdin").write_all(input)?;
        }
        let mut output = Vec::new();
        child
            .stdout
            .take()
            .expect("piped stdout")
            .take(MAX_RESPONSE as u64 + 1)
            .read_to_end(&mut output)?;
        if output.len() > MAX_RESPONSE {
            return Err(auth_error());
        }
        Ok((child.wait()?.code().unwrap_or(-1), output))
    })();
    if collected.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    collected
}

fn auth_error() -> io::Error {
    io::Error::other("Could not finish secure ChatGPT sign-in.")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attempt(client: Option<&str>) -> Attempt {
        Attempt {
            state: "attempt".into(),
            nonce: "nonce".into(),
            verifier: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into(),
            redirect: "http://127.0.0.1:12345/auth/callback".into(),
            client: client.map(str::to_owned),
        }
    }

    #[test]
    fn pkce_and_callback_bind_the_issued_client_and_attempt() {
        let pending = attempt(None);
        let url = pending.url("urn:uuid:host");
        let query: std::collections::HashMap<_, _> = url.query_pairs().collect();
        assert_eq!(
            query["code_challenge"],
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        assert_eq!(query["redirect_uri"], pending.redirect);
        assert_eq!(query["client_id"], DYNAMIC_CLIENT);
        assert_eq!(query["agent_name_hint"], "Offish");
        assert!(
            pending
                .callback("/auth/callback?code=x&state=another&client_id=oaiapp_example")
                .is_none()
        );
        assert!(
            pending
                .callback("/auth/callback?code=x&state=attempt&state=attempt")
                .is_none()
        );
        assert!(
            pending
                .callback("/auth/callback?code=x&state=attempt")
                .unwrap()
                .is_err()
        );
        assert!(
            pending
                .callback("/auth/callback?error=access_denied&state=attempt")
                .unwrap()
                .is_err()
        );
        assert_eq!(
            pending
                .callback("/auth/callback?code=x&state=attempt&client_id=oaiapp_example")
                .unwrap()
                .unwrap()
                .client,
            "oaiapp_example"
        );
        let saved = attempt(Some("oaiapp_saved"));
        assert!(
            saved
                .callback("/auth/callback?code=x&state=attempt&client_id=oaiapp_other")
                .unwrap()
                .is_err()
        );
        assert_eq!(
            saved
                .callback("/auth/callback?code=x&state=attempt")
                .unwrap()
                .unwrap()
                .client,
            "oaiapp_saved"
        );
        assert!(
            !saved
                .url("urn:uuid:host")
                .query_pairs()
                .any(|(key, _)| key == "agent_name_hint")
        );
    }

    #[test]
    fn identity_token_requires_signature_nonce_and_audience() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("identity-test.json")).unwrap();
        let keys: Keys = serde_json::from_value(fixture["jwks"].clone()).unwrap();
        let token = fixture["token"].as_str().unwrap();
        assert_eq!(
            verify_identity(token, &keys, "oaiapp_test", "signed-nonce").unwrap(),
            "fixture-user"
        );
        assert!(verify_identity(token, &keys, "oaiapp_test", "other-nonce").is_err());
        assert!(verify_identity(token, &keys, "oaiapp_other", "signed-nonce").is_err());
        let (signed, _) = token.rsplit_once('.').unwrap();
        assert!(
            verify_identity(
                &format!("{signed}.invalid"),
                &keys,
                "oaiapp_test",
                "signed-nonce"
            )
            .is_err()
        );
    }

    #[test]
    fn identity_scope_alone_is_insufficient_and_refresh_rotates_credentials() {
        let mut tokens = Tokens {
            access_token: "fixture-access".into(),
            refresh_token: Some("fixture-refresh".into()),
            id_token: None,
            scope: Some("openid email".into()),
            token_type: "Bearer".into(),
            expires_in: 600,
        };
        assert!(account(tokens, "oaiapp_example".into(), "sample".into(), None).is_err());
        tokens = Tokens {
            access_token: "first-access".into(),
            refresh_token: Some("first-refresh".into()),
            id_token: None,
            scope: Some(SCOPES.into()),
            token_type: "Bearer".into(),
            expires_in: 600,
        };
        let prior = account(tokens, "oaiapp_example".into(), "sample".into(), None).unwrap();
        let replacement = Tokens {
            access_token: "second-access".into(),
            refresh_token: Some("second-refresh".into()),
            id_token: None,
            scope: None,
            token_type: "Bearer".into(),
            expires_in: 600,
        };
        let current = account(
            replacement,
            prior.client.clone(),
            prior.subject.clone(),
            Some(prior),
        )
        .unwrap();
        assert_eq!(current.token, "second-access");
        assert_eq!(current.refresh, "second-refresh");
        assert_eq!(current.scope, SCOPES);
    }
}
