//! Purpose: Everything that talks to GitHub about an update and decides whether to trust it: the release list, version order, the one allowed installer address and the signature check.
//! Contents: Manifest — the release list entry; triple / newer — version comparison; get — GET that follows only https redirects by hand; installer_url — the one URL an installer may come from; fetch_manifest — read `latest.json`; verify_reader / verify_file — minisign check against the key built in here.

use std::io::Read;
use std::path::Path;
use std::time::Duration;

use base64::Engine;
use minisign_verify::{PublicKey, Signature};
use serde::Deserialize;

const MANIFEST_URL: &str = "https://github.com/Rishik15/cue/releases/latest/download/latest.json";
/// The installer must come from this repository's releases, whatever the list says: `installer_url` builds the one allowed address.
const DOWNLOADS: &str = "https://github.com/Rishik15/cue/releases/download/";
/// Base64 of the minisign public key made by `tauri signer generate`; the private key signs each release installer.
const PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDJGNEVDQ0VENUI2Mzk4NzAKUldSd21HTmI3Y3hPTCt1WDBSZzdZY3psU0pyRlB0TjdnOFhUeFBJV1lnaUNjR1ptSkZ1akNsamYK";
const MAX_HOPS: usize = 5;

#[derive(Clone, Deserialize)]
pub(super) struct Manifest {
    pub(super) version: String,
    pub(super) url: String,
    pub(super) signature: String,
}

/// The whole body (a list, or an installer of about 11 MB) may take this long: the HTTP client has no timeout per read, so this is what cuts off a stalled connection.
const BODY_TIME: Duration = Duration::from_secs(15 * 60);

type Response = ureq::http::Response<ureq::Body>;

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .max_redirects(0)
        .max_redirects_will_error(false)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .timeout_recv_body(Some(BODY_TIME))
        .build()
        .into()
}

fn triple(version: &str) -> Option<[u64; 3]> {
    let mut parts = version.trim_start_matches('v').split('.').map(|p| p.parse().ok());
    Some([parts.next()??, parts.next()??, parts.next()??])
}

/// Only a strictly newer version counts; an unreadable one never does.
pub(super) fn newer(remote: &str, local: &str) -> bool {
    matches!((triple(remote), triple(local)), (Some(r), Some(l)) if r > l)
}

/// A redirect may only lead to another https address (never plain http, never a relative path).
fn redirect_ok(location: &str) -> bool {
    location.starts_with("https://")
}

/// GET that follows up to `MAX_HOPS` redirects by hand. Err carries the HTTP status when there was one.
pub(super) fn get(url: &str) -> Result<Response, Option<u16>> {
    let agent = agent();
    let mut url = url.to_string();
    for _ in 0..=MAX_HOPS {
        let response = match agent.get(&url).call() {
            Ok(response) => response,
            Err(ureq::Error::StatusCode(code)) => return Err(Some(code)),
            Err(_) => return Err(None),
        };
        if !response.status().is_redirection() {
            return Ok(response);
        }
        match response.headers().get("location").and_then(|v| v.to_str().ok()) {
            Some(next) if redirect_ok(next) => url = next.to_string(),
            _ => return Err(None),
        }
    }
    Err(None)
}

/// The only address an installer for `version` may have; None when the version is not a plain `x.y.z`.
pub(super) fn installer_url(version: &str) -> Option<String> {
    let [a, b, c] = triple(version)?;
    (version == format!("{a}.{b}.{c}")).then(|| format!("{DOWNLOADS}v{version}/Cue_{version}_x64-setup.exe"))
}

/// None when the newest release carries no list at all: nothing newer has been published for this Cue to take.
pub(super) fn fetch_manifest() -> Result<Option<Manifest>, String> {
    let response = match get(MANIFEST_URL) {
        Ok(response) => response,
        Err(Some(404)) => return Ok(None),
        Err(_) => return Err("Could not reach GitHub to check for updates.".into()),
    };
    let mut body = String::new();
    response
        .into_body()
        .into_reader()
        .take(64 * 1024)
        .read_to_string(&mut body)
        .map_err(|e| format!("Could not read the update list: {e}"))?;
    serde_json::from_str(&body).map(Some).map_err(|_| "The update list is not readable.".to_string())
}

/// Checks `data` against the signature in chunks, so a large installer never sits in memory. Only the prehashed signatures `tauri signer` makes are accepted.
fn verify_reader(mut data: impl Read, signature: &str) -> Result<(), String> {
    const BAD: &str = "The update could not be verified, so it was not installed.";
    let text =
        |encoded: &str| base64::engine::general_purpose::STANDARD.decode(encoded.trim()).ok().and_then(|b| String::from_utf8(b).ok());
    let key = PublicKey::decode(&text(PUBLIC_KEY).ok_or(BAD)?).map_err(|_| BAD)?;
    let signature = Signature::decode(&text(signature).ok_or(BAD)?).map_err(|_| BAD)?;
    let mut check = key.verify_stream(&signature).map_err(|_| BAD)?;
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        match data.read(&mut chunk).map_err(|_| BAD)? {
            0 => break,
            n => check.update(&chunk[..n]),
        }
    }
    check.finalize().map_err(|_| BAD.to_string())
}

/// Verifies the file as it is on disk right now.
pub(super) fn verify_file(path: &Path, signature: &str) -> Result<(), String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Could not read the saved update: {e}"))?;
    verify_reader(file, signature)
}
