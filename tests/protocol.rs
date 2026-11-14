use aihmc_api::{protocol, ucenter};
use base64::{Engine, engine::general_purpose::STANDARD};
use rsa::{Pkcs1v15Sign, RsaPrivateKey, RsaPublicKey, pkcs8::DecodePublicKey, rand_core::OsRng};
use serde_json::Value;
use sha1::{Digest, Sha1};

#[test]
fn matches_independent_legacy_vectors_byte_for_byte() {
    let vector: Value =
        serde_json::from_str(include_str!("../docs/legacy-protocol-vectors.json")).unwrap();
    let timestamp = vector["timestamp"].as_i64().unwrap();
    let name = vector["username"].as_str().unwrap();
    let client = vector["clientToken"].as_str().unwrap();
    assert_eq!(
        protocol::access_token(name, client, timestamp),
        vector["accessToken"]
    );
    assert_eq!(
        protocol::refreshed_token(client, vector["refreshTimestamp"].as_i64().unwrap()),
        vector["refreshToken"]
    );
    assert_eq!(
        protocol::profile_id(
            name,
            vector["email"].as_str().unwrap(),
            vector["uid"].as_i64().unwrap()
        ),
        vector["profileId"]
    );
    assert_eq!(
        ucenter::signature(
            vector["ucenter"]["nonce"].as_str().unwrap(),
            timestamp,
            vector["ucenter"]["secret"].as_str().unwrap()
        ),
        vector["ucenter"]["signature"]
    );
    assert_eq!(
        protocol::encode_texture(
            timestamp,
            "fixture-id",
            "迁移测试",
            "fixture-skin",
            "slim",
            "fixture-cape",
            "https://api.aihmc.top/textures"
        )
        .unwrap(),
        vector["textureValue"]
    );
    let public =
        RsaPublicKey::from_public_key_pem(vector["rsa"]["publicKey"].as_str().unwrap()).unwrap();
    public
        .verify(
            Pkcs1v15Sign::new::<Sha1>(),
            &Sha1::digest(vector["rsa"]["value"].as_str().unwrap()),
            &STANDARD
                .decode(vector["rsa"]["signature"].as_str().unwrap())
                .unwrap(),
        )
        .unwrap();
}

#[test]
fn preserves_identifier_format_signature_and_wire_shape() {
    let uuid = protocol::new_uuid().unwrap();
    assert_eq!(uuid.len(), 32);
    assert!(
        uuid.bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    );
    assert_eq!(&uuid[12..13], "4");
    assert!(matches!(&uuid[16..17], "8" | "9" | "a" | "b"));
    assert_eq!(protocol::client_token().unwrap().len(), 32);
    let key = RsaPrivateKey::new(&mut OsRng, 1024).unwrap();
    let value = "existing-base64-value";
    let signature = STANDARD
        .decode(protocol::sign(value, &key).unwrap())
        .unwrap();
    key.to_public_key()
        .verify(
            Pkcs1v15Sign::new::<Sha1>(),
            &Sha1::digest(value),
            &signature,
        )
        .unwrap();
    let profile = protocol::Profile {
        id: "fixture-id".into(),
        name: "迁移测试".into(),
        properties: vec![],
    };
    let login = protocol::Login {
        access_token: "a".into(),
        client_token: "c".into(),
        available_profiles: vec![profile.clone()],
        selected_profile: Some(profile),
        user: Some(protocol::User {
            id: "user-id".into(),
            properties: vec![],
        }),
    };
    let json = serde_json::to_value(login).unwrap();
    assert!(json["selectedProfile"].get("properties").is_none());
    assert_eq!(json["user"]["properties"], serde_json::json!([]));
    for (text, truth) in [("", false), ("0", false), ("false", true), ("true", true)] {
        assert_eq!(protocol::php_truthy_query(text), truth);
    }
    let empty =
        protocol::encode_texture(1, "id", "中文", "", "", "", "https://example.com").unwrap();
    let decoded: Value = serde_json::from_slice(&STANDARD.decode(empty).unwrap()).unwrap();
    assert!(decoded.get("textures").is_none());
}

#[test]
#[ignore = "requires Windows Go protocol bridge and synthetic runtime config"]
fn verifies_go_and_rust_signatures_in_both_directions() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let bridge = std::env::var("AIHMC_GO_PROTOCOL_BRIDGE").expect("Go bridge executable required");
    let config = std::env::var("AIHMC_TEST_CONFIG").expect("isolated test config required");
    let runtime = aihmc_api::config::RuntimeConfig::load(std::path::Path::new(&config)).unwrap();
    let value = "synthetic-base64-texture-中文";
    let rust_signature = protocol::sign(value, &runtime.private_key).unwrap();
    let mut child = Command::new(bridge)
        .args(["--config", &config])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            serde_json::to_string(
                &serde_json::json!({"value": value, "signature": rust_signature}),
            )
            .unwrap()
            .as_bytes(),
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["verified"], true);
    let go_signature = STANDARD
        .decode(response["signature"].as_str().unwrap())
        .unwrap();
    runtime
        .private_key
        .to_public_key()
        .verify(
            Pkcs1v15Sign::new::<Sha1>(),
            &Sha1::digest(value),
            &go_signature,
        )
        .unwrap();
    assert_eq!(response["signature"], rust_signature);
}
