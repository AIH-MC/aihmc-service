use std::{io::Cursor, sync::LazyLock, time::Duration};

use image::{ImageFormat, Rgba, RgbaImage, imageops::FilterType};
use regex::Regex;

use crate::{error::ApiResult, skin, state::AppState, upstream};

pub fn process_skin(bytes: &[u8], size: u32) -> Option<Vec<u8>> {
    if size == 0 {
        return None;
    }
    let source = image::load_from_memory(bytes).ok()?.into_rgba8();
    let mut head = RgbaImage::new(8, 8);
    for y in 0..8 {
        for x in 0..8 {
            let mut pixel = source
                .get_pixel_checked(x + 8, y + 8)
                .copied()
                .unwrap_or(Rgba([0; 4]));
            if source.width() >= 64 {
                let overlay = source
                    .get_pixel_checked(x + 40, y + 8)
                    .copied()
                    .unwrap_or(Rgba([0; 4]));
                let mask = u32::from(overlay[3]);
                // Pillow paste(mask=overlay) blends all four channels, including alpha.
                for channel in 0..4 {
                    pixel[channel] = ((u32::from(pixel[channel]) * (255 - mask)
                        + u32::from(overlay[channel]) * mask
                        + 127)
                        / 255) as u8;
                }
            }
            head.put_pixel(x, y, pixel);
        }
    }
    let resized = image::imageops::resize(&head, size, size, FilterType::Nearest);
    let mut output = Cursor::new(Vec::new());
    resized.write_to(&mut output, ImageFormat::Png).ok()?;
    Some(output.into_inner())
}

pub fn identify(value: &str) -> &'static str {
    static UUID: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^([0-9a-fA-F]{32}|[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})$").unwrap()
    });
    if UUID.is_match(value) {
        return "UUID";
    }
    let length = value.chars().count();
    if (1..=16).contains(&length)
        && value
            .chars()
            .all(|c| c == '_' || c.is_alphanumeric() || ('\u{0080}'..='\u{d7ff}').contains(&c))
    {
        "USERNAME"
    } else {
        "UNKNOWN"
    }
}

async fn player_profile(state: &AppState, account: &str) -> ApiResult<Option<serde_json::Value>> {
    let mut uuid = match identify(account) {
        "UUID" => Some(account.replace('-', "")),
        "USERNAME" => None,
        _ => return Ok(None),
    };
    if uuid.is_none() {
        for source in state.config.servers.iter().filter(|s| s.is_enabled()) {
            let url = format!("{}/{account}", upstream::endpoint(source, "name")?);
            if let Some(profile) = upstream::get_json(state, &url).await? {
                uuid = profile["id"].as_str().map(|s| s.replace('-', ""));
                break;
            }
        }
    }
    let Some(uuid) = uuid else {
        return Ok(None);
    };
    for source in state.config.servers.iter().filter(|s| s.is_enabled()) {
        let url = format!("{}/{uuid}", upstream::endpoint(source, "head-profile")?);
        if let Some(profile) = upstream::get_json(state, &url).await? {
            return Ok(Some(profile));
        }
    }
    Ok(None)
}

async fn default_head(state: &AppState, size: u32) -> Option<Vec<u8>> {
    let bytes = tokio::fs::read(state.resource("assets/alex.png"))
        .await
        .ok()?;
    process_skin(&bytes, size)
}

pub async fn get_head(state: &AppState, account: &str, size: u32) -> ApiResult<Option<Vec<u8>>> {
    let profile = player_profile(state, account).await?;
    let url = profile
        .as_ref()
        .and_then(skin::decode_textures)
        .and_then(|v| v["textures"]["SKIN"]["url"].as_str().map(String::from));
    let Some(url) = url else {
        return Ok(default_head(state, size).await);
    };
    let download = async {
        let response = state
            .client
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await?;
        if response.status() != reqwest::StatusCode::OK {
            return Ok(None);
        }
        response.bytes().await.map(Some)
    }
    .await;
    match download {
        Ok(Some(bytes)) => {
            if let Some(head) = process_skin(&bytes, size) {
                Ok(Some(head))
            } else {
                Ok(default_head(state, size).await)
            }
        }
        Ok(None) => Ok(default_head(state, size).await),
        Err(error) => {
            tracing::error!(%error, "head texture download failed");
            Ok(default_head(state, size).await)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_blending_matches_pillow_mask_behavior() {
        let mut image = RgbaImage::new(64, 64);
        for y in 8..16 {
            for x in 8..16 {
                image.put_pixel(x, y, Rgba([200, 0, 0, 255]));
            }
            for x in 40..48 {
                image.put_pixel(x, y, Rgba([0, 100, 0, 128]));
            }
        }
        let mut source = Cursor::new(Vec::new());
        image.write_to(&mut source, ImageFormat::Png).unwrap();
        let png = process_skin(source.get_ref(), 16).unwrap();
        let decoded = image::load_from_memory(&png).unwrap().into_rgba8();
        assert_eq!(decoded.dimensions(), (16, 16));
        assert_eq!(decoded.get_pixel(0, 0).0, [100, 50, 0, 191]);
        assert!(process_skin(b"invalid", 16).is_none());
        assert!(process_skin(source.get_ref(), 0).is_none());
        assert_eq!(identify("玩家"), "USERNAME");
        assert_eq!(identify("12345678-1234-1234-1234-123456789abc"), "UUID");
    }
}
