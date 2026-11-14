use base64::{Engine, engine::general_purpose::STANDARD};
use rsa::{
    Pkcs1v15Sign, RsaPrivateKey,
    rand_core::{OsRng, RngCore},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha1::{Digest, Sha1};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Property {
    pub name: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signature: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Profile {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<Property>,
}

#[derive(Serialize)]
pub struct User {
    pub id: String,
    pub properties: Vec<Property>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Login {
    pub access_token: String,
    pub client_token: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub available_profiles: Vec<Profile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_profile: Option<Profile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
}

fn md5_hex(value: &str) -> String {
    format!("{:x}", md5::compute(value))
}

pub fn access_token(name: &str, client: &str, timestamp: i64) -> String {
    STANDARD.encode(format!(
        "{}{}",
        md5_hex(&format!("{name}{}", md5_hex(client))),
        md5_hex(&timestamp.to_string())
    ))
}

pub fn refreshed_token(client: &str, timestamp: i64) -> String {
    let encoded = STANDARD.encode(md5_hex(client));
    STANDARD.encode(md5_hex(&format!(
        "{encoded}{}",
        md5_hex(&timestamp.to_string())
    )))
}

pub fn profile_id(name: &str, email: &str, uid: i64) -> String {
    md5_hex(&format!("{name}{email}{}", md5_hex(&uid.to_string())))
}

pub fn client_token() -> Result<String, rsa::rand_core::Error> {
    let mut bytes = [0_u8; 16];
    OsRng.try_fill_bytes(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub fn new_uuid() -> Result<String, rsa::rand_core::Error> {
    let mut bytes = [0_u8; 16];
    OsRng.try_fill_bytes(&mut bytes)?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub fn sign(value: &str, key: &RsaPrivateKey) -> Result<String, rsa::Error> {
    key.sign(Pkcs1v15Sign::new::<Sha1>(), &Sha1::digest(value.as_bytes()))
        .map(|signature| STANDARD.encode(signature))
}

pub fn php_truthy_query(value: &str) -> bool {
    !value.is_empty() && value != "0"
}

pub fn encode_texture(
    timestamp: i64,
    uuid: &str,
    name: &str,
    skin: &str,
    model: &str,
    cape: &str,
    base_url: &str,
) -> Result<String, serde_json::Error> {
    let mut payload = json!({"timestamp": timestamp, "profileId": uuid, "profileName": name});
    if !skin.is_empty() || !cape.is_empty() {
        let mut textures = serde_json::Map::new();
        if !skin.is_empty() {
            textures.insert(
                "SKIN".into(),
                json!({
                    "url": format!("{}/{}", base_url.trim_end_matches('/'), skin),
                    "metadata": {"model": model},
                }),
            );
        }
        if !cape.is_empty() {
            textures.insert(
                "CAPE".into(),
                json!({"url": format!("{}/{}", base_url.trim_end_matches('/'), cape)}),
            );
        }
        payload["textures"] = serde_json::Value::Object(textures);
    }
    let mut output = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut serializer = serde_json::Serializer::with_formatter(&mut output, formatter);
    payload.serialize(&mut serializer)?;
    // PHP's pretty JSON keeps Unicode but escapes forward slashes before Base64.
    let text = String::from_utf8(output)
        .expect("JSON serialization is UTF-8")
        .replace('/', "\\/");
    Ok(STANDARD.encode(text))
}
