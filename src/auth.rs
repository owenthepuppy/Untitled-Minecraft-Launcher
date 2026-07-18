use serde::Deserialize;
use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    thread::sleep,
    time::Duration,
};
use ureq::Agent;

// to anyone trying to make something similar, you need your own client ID, you can get one from like an azure project or something, and then it won't work and the error code from some later xbox or minecraft api will say to go to another link where you fill a form and hope they let you use the api, i managed to get it, but don't hope on it.
const CLIENT_ID: &str = "daa4feeb-ac95-4ee0-b6e2-2041746b9a50";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const DEVICECODE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const XBL_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
#[derive(Deserialize)]
struct Device {
    device_code: String,
    user_code: String,
    verification_uri: String,
    interval: u64,
}
#[derive(Deserialize)]
struct XblResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: DisplayClaims,
}

#[derive(Deserialize)]
struct DisplayClaims {
    xui: Vec<Xui>,
}

#[derive(Deserialize)]
struct Xui {
    uhs: String,
    #[serde(default)]
    xid: Option<String>,
}
#[derive(Deserialize)]
struct McTokenResponse {
    access_token: String,
}
#[derive(Deserialize)]
struct Profile {
    id: String,
    name: String,
}
#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    error: Option<String>,
}
pub struct Account {
    pub name: String,
    pub uuid: String,
    pub access_token: String,
    pub xuid: String,
}

fn token_path() -> anyhow::Result<PathBuf> {
    let home = std::env::var("HOME")?;
    Ok(Path::new(&home).join(".config").join("uml").join("token"))
}

fn save_refresh(token: &str) -> anyhow::Result<()> {
    let path = token_path()?;
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, token)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

fn agent() -> Agent {
    Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .new_agent()
}
fn device_code(agent: &Agent) -> anyhow::Result<Device> {
    let mut res = agent.post(DEVICECODE_URL).send_form([
        ("client_id", CLIENT_ID),
        ("scope", "XboxLive.signin offline_access"),
    ])?;
    if res.status() != 200 {
        let body = res.body_mut().read_to_string()?;
        anyhow::bail!("device code failed ({}): {body}", res.status());
    }
    Ok(res.body_mut().read_json()?)
}
fn refresh(agent: &Agent, refresh: &str) -> anyhow::Result<(String, String)> {
    let mut res = agent.post(TOKEN_URL).send_form([
        ("grant_type", "refresh_token"),
        ("client_id", CLIENT_ID),
        ("refresh_token", refresh),
    ])?;
    if res.status() != 200 {
        let body = res.body_mut().read_to_string()?;
        anyhow::bail!("refresh failed ({}): {body}", res.status());
    }
    let body: TokenResponse = res.body_mut().read_json()?;
    let access = body
        .access_token
        .ok_or_else(|| anyhow::anyhow!("no access token"))?;
    let new_refresh = body
        .refresh_token
        .ok_or_else(|| anyhow::anyhow!("no refresh token"))?;
    Ok((access, new_refresh))
}
fn poll(agent: &Agent, device: &Device) -> anyhow::Result<(String, String)> {
    println!(
        "Go to {} and enter code: {}",
        device.verification_uri, device.user_code
    );
    loop {
        sleep(Duration::from_secs(device.interval));

        let mut res = agent.post(TOKEN_URL).send_form([
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", CLIENT_ID),
            ("device_code", device.device_code.as_str()),
        ])?;
        let body: TokenResponse = res.body_mut().read_json()?;

        if let Some(token) = body.access_token {
            let refresh = body.refresh_token.ok_or_else(|| {
                anyhow::anyhow!("no refresh token — was offline_access in the scope?")
            })?;
            return Ok((token, refresh));
        }
        match body.error.as_deref() {
            Some("authorization_pending") => continue,
            Some("slow_down") => {
                sleep(Duration::from_secs(5));
                continue;
            }
            Some(other) => anyhow::bail!("auth failed: {other}"),
            None => anyhow::bail!("unexpected response"),
        }
    }
}
fn xbl(agent: &Agent, ms_token: &str) -> anyhow::Result<XblResponse> {
    let mut res = agent.post(XBL_URL).send_json(serde_json::json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": format!("d={ms_token}")
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    }))?;
    if res.status() != 200 {
        let body = res.body_mut().read_to_string()?;
        anyhow::bail!("xbl failed ({}): {body}", res.status());
    }
    Ok(res.body_mut().read_json()?)
}
fn xsts(agent: &Agent, xbl_token: &str) -> anyhow::Result<XblResponse> {
    let mut res = agent.post(XSTS_URL).send_json(serde_json::json!({
        "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT"
    }))?;
    if res.status() == 401 {
        #[derive(Deserialize)]
        struct XErr {
            #[serde(rename = "XErr")]
            xerr: u64,
        }
        let e: XErr = res.body_mut().read_json()?;
        anyhow::bail!(
            "{}",
            match e.xerr {
                2148916233 => "no Xbox profile — sign in at minecraft.net once".into(),
                2148916235 => "Xbox Live unavailable in this region".into(),
                2148916237 => "account needs adult verification".into(),
                2148916238 => "child account — must be added to a Family".into(),
                other => format!("XSTS failed, XErr {other}"),
            }
        );
    }
    if res.status() != 200 {
        let body = res.body_mut().read_to_string()?;
        anyhow::bail!("xsts failed ({}): {body}", res.status());
    }
    Ok(res.body_mut().read_json()?)
}
fn mc_token(agent: &Agent, uhs: &str, xsts_token: &str) -> anyhow::Result<String> {
    let mut res = agent.post(MC_LOGIN_URL).send_json(serde_json::json!({
        "identityToken": format!("XBL3.0 x={uhs};{xsts_token}")
    }))?;
    if res.status() != 200 {
        let body = res.body_mut().read_to_string()?;
        anyhow::bail!("mc login failed ({}): {body}", res.status());
    }
    let t: McTokenResponse = res.body_mut().read_json()?;
    Ok(t.access_token)
}
fn profile(agent: &Agent, mc_token: &str) -> anyhow::Result<Profile> {
    let mut res = agent
        .get(PROFILE_URL)
        .header("Authorization", &format!("Bearer {mc_token}"))
        .call()?;
    if res.status() == 404 {
        anyhow::bail!("this account doesn't own Minecraft Java");
    }
    if res.status() != 200 {
        let body = res.body_mut().read_to_string()?;
        anyhow::bail!("profile failed ({}): {body}", res.status());
    }
    Ok(res.body_mut().read_json()?)
}
pub fn login() -> anyhow::Result<Account> {
    let agent = agent();

    let (ms, refresh) = match std::fs::read_to_string(token_path()?) {
        Ok(cached) => match refresh(&agent, cached.trim()) {
            Ok(pair) => pair,
            Err(_) => {
                // expired or revoked
                let device = device_code(&agent)?;
                poll(&agent, &device)?
            }
        },
        Err(_) => {
            // no file yet
            let device = device_code(&agent)?;
            poll(&agent, &device)?
        }
    };
    save_refresh(&refresh)?;
    let xbl = xbl(&agent, &ms)?;
    let uhs = xbl.display_claims.xui[0].uhs.clone();
    let xuid = xbl.display_claims.xui[0].xid.clone().unwrap_or_default();

    let xsts = xsts(&agent, &xbl.token)?;
    let mc = mc_token(&agent, &uhs, &xsts.token)?;
    let p = profile(&agent, &mc)?;

    Ok(Account {
        name: p.name,
        uuid: p.id,
        access_token: mc,
        xuid,
    })
}

// pub fn gen_offline(name: &str) -> Account {
//     let uuid = uuid::Uuid::new_v3(
//         &uuid::Uuid::NAMESPACE_OID,
//         format!("OfflinePlayer:{name}").as_bytes(),
//     );
//     Account {
//         name: name.to_string(),
//         uuid: uuid.simple().to_string(),
//         access_token: "0".to_string(),
//         xuid: String::new(),
//     }
// }
