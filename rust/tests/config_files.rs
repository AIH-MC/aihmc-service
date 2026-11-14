use std::fs;

use aihmc_api::config::RuntimeConfig;
use rsa::{
    RsaPrivateKey,
    pkcs1::{EncodeRsaPrivateKey, EncodeRsaPublicKey},
    pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding},
    rand_core::OsRng,
};

fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    for name in ["keys", "secrets", "textures"] {
        fs::create_dir(directory.path().join(name)).unwrap();
    }
    let key = RsaPrivateKey::new(&mut OsRng, 1024).unwrap();
    fs::write(
        directory.path().join("keys/yggdrasil-private-key.pem"),
        key.to_pkcs1_pem(LineEnding::LF).unwrap().as_bytes(),
    )
    .unwrap();
    fs::write(
        directory.path().join("keys/yggdrasil-public-key.pem"),
        key.to_public_key()
            .to_public_key_pem(LineEnding::LF)
            .unwrap(),
    )
    .unwrap();
    fs::write(
        directory.path().join("secrets/database-password"),
        "  synthetic-db-value  \r\n",
    )
    .unwrap();
    fs::write(
        directory.path().join("secrets/ucenter-secret"),
        "synthetic-ucenter-value\n",
    )
    .unwrap();
    fs::write(
        directory.path().join("secrets/admin-super-key"),
        "synthetic-admin-value",
    )
    .unwrap();
    let path = directory.path().join("config.toml");
    fs::write(&path, include_str!("../../config.example.toml")).unwrap();
    (directory, path)
}

#[test]
fn resolves_resources_relative_to_config_and_preserves_secrets() {
    let (directory, path) = fixture();
    let runtime = RuntimeConfig::load(&path).unwrap();
    assert_eq!(runtime.database_password, "  synthetic-db-value  ");
    assert_eq!(runtime.ucenter_secret, "synthetic-ucenter-value");
    assert_eq!(runtime.super_key, "synthetic-admin-value");
    assert_eq!(
        runtime.config.textures.directory,
        directory.path().join("textures")
    );
    assert_eq!(
        fs::read_dir(&runtime.config.textures.directory)
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn accepts_pkcs8_private_and_pkcs1_public_keys() {
    let (_directory, path) = fixture();
    let key = RsaPrivateKey::new(&mut OsRng, 1024).unwrap();
    let base = path.parent().unwrap();
    fs::write(
        base.join("keys/yggdrasil-private-key.pem"),
        key.to_pkcs8_pem(LineEnding::LF).unwrap().as_bytes(),
    )
    .unwrap();
    fs::write(
        base.join("keys/yggdrasil-public-key.pem"),
        key.to_public_key().to_pkcs1_pem(LineEnding::LF).unwrap(),
    )
    .unwrap();
    assert!(RuntimeConfig::load(&path).is_ok());
}

#[test]
fn rejects_missing_secret_and_mismatched_keys() {
    let (directory, path) = fixture();
    let key = RsaPrivateKey::new(&mut OsRng, 1024).unwrap();
    fs::write(
        directory.path().join("keys/yggdrasil-public-key.pem"),
        key.to_public_key()
            .to_public_key_pem(LineEnding::LF)
            .unwrap(),
    )
    .unwrap();
    assert!(
        RuntimeConfig::load(&path)
            .err()
            .unwrap()
            .contains("do not match")
    );
    fs::remove_file(directory.path().join("secrets/database-password")).unwrap();
    assert!(
        RuntimeConfig::load(&path)
            .err()
            .unwrap()
            .contains("database.password_file")
    );
}
