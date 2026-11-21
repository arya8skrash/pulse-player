//! Online SoundCloud search and playback.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::State;

use crate::{audio::AudioCmd, db::Track, state::AppState};

const API_BASE: &str = "https://music.youtube.com/youtubei/v1";
const YOUTUBE_API_BASE: &str = "https://www.youtube.com/youtubei/v1";

// Client identity constants
const CLIENT_NAME: &str = "WEB_REMIX";
const CLIENT_ID: &str = "67"; // WEB_REMIX ID
const YOUTUBE_WEB_CLIENT_NAME: &str = "WEB";
const YOUTUBE_WEB_CLIENT_ID: &str = "1";
const YOUTUBE_EMBEDDED_CLIENT_NAME: &str = "WEB_EMBEDDED_PLAYER";
const YOUTUBE_EMBEDDED_CLIENT_ID: &str = "56";

// API Key used by the YouTube Music web client.
const FALLBACK_API_KEY: &str = "AIzaSyC9XL3ZjWddXya6X74dJoCTL-WEYFDNX30";
const FALLBACK_CLIENT_VERSION: &str = "1.20260707.12.00";
const FALLBACK_YOUTUBE_WEB_CLIENT_VERSION: &str = "2.20260707.00.00";

const WEB_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const EMBEDDED_USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/15.5 Safari/605.1.15,gzip(gfe)";

// Filter param for the "Songs" tab of YouTube Music search.
const SONGS_FILTER: &str = "EgWKAQIIAWoKEAoQAxAEEAkQBQ==";
// Video results are a useful fallback when YouTube Music has no catalog-song
// matches for a query.
const VIDEOS_FILTER: &str = "EgWKAQIQAWoKEAoQAxAEEAkQBQ==";

// Cached visitorData. It is optional: YouTube can issue a fresh value in its
// response when scraping the homepage is unavailable.
static VISITOR_DATA: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));
static BOTGUARD_MINT_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

#[derive(Debug, Clone)]
struct InnertubeConfig {
    api_key: String,
    youtube_api_key: String,
    client_version: String,
    youtube_web_client_version: String,
    signature_timestamp: Option<u64>,
    visitor_data: Option<String>,
}

static INNERTUBE_CONFIG: Lazy<Mutex<Option<InnertubeConfig>>> = Lazy::new(|| Mutex::new(None));

static USE_SYSTEM_PROXY: AtomicBool = AtomicBool::new(false);
static SOUNDCLOUD_CLIENT_ID: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

static YOUTUBE_COOKIE_JAR: Lazy<Mutex<Arc<reqwest::cookie::Jar>>> =
    Lazy::new(|| Mutex::new(new_youtube_cookie_jar()));

fn new_youtube_cookie_jar() -> Arc<reqwest::cookie::Jar> {
    let jar = Arc::new(reqwest::cookie::Jar::default());
    if let Ok(url) = reqwest::Url::parse("https://music.youtube.com/") {
        jar.add_cookie_str(
            "SOCS=CAESHAgBEhJnd3NfMjAyMzA4MTAtMF9SQzIaAmVuIAEaBgiA_LyaBg; Path=/; Secure",
            &url,
        );
    }
    jar
}

fn youtube_cookie_jar() -> Arc<reqwest::cookie::Jar> {
    YOUTUBE_COOKIE_JAR
        .lock()
        .map(|jar| Arc::clone(&jar))
        .unwrap_or_else(|poisoned| Arc::clone(&poisoned.into_inner()))
}

fn reset_youtube_guest_session() {
    if let Ok(mut jar) = YOUTUBE_COOKIE_JAR.lock() {
        *jar = new_youtube_cookie_jar();
    }
    if let Ok(mut visitor_data) = VISITOR_DATA.lock() {
        *visitor_data = None;
    }
    if let Ok(mut config) = INNERTUBE_CONFIG.lock() {
        *config = None;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineTrack {
    pub video_id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_secs: u32,
    pub thumbnail_url: Option<String>,
}

fn build_client(user_agent: &str) -> Result<reqwest::Client, String> {
    build_client_with_timeout(user_agent, Duration::from_secs(20))
}

fn build_client_with_timeout(
    user_agent: &str,
    timeout: Duration,
) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        .timeout(timeout)
        .user_agent(user_agent)
        .cookie_provider(youtube_cookie_jar());

    // Leave Reqwest's automatic platform proxy enabled when requested. Adding
    // an explicit Proxy overrides that behavior, and reading only proxy
    // environment variables misses the Windows system proxy configuration.
    builder = if USE_SYSTEM_PROXY.load(Ordering::Acquire) {
        log::debug!("Online requests are using Reqwest's system proxy detection");
        builder
    } else {
        builder.no_proxy()
    };

    builder
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))
}

/// Mint the two Web PO tokens YouTube expects: a session-bound token for GVS
/// (bound to visitorData) and a video-bound token for the /player request.
/// The challenge request uses the same proxy and guest cookie jar as other
/// online requests, and a snapshot keeps later token generation fast.
async fn mint_playback_po_tokens(
    video_id: String,
    visitor_data: String,
) -> Result<(String, String), String> {
    let cache_dir = std::env::temp_dir().join("pulse-online-cache");
    fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("Could not create the online music cache: {e}"))?;
    let snapshot_path = cache_dir.join("botguard.snapshot");
    let client = build_client_with_timeout(WEB_USER_AGENT, Duration::from_secs(45))?;

    tauri::async_runtime::spawn_blocking(move || {
        let _guard = BOTGUARD_MINT_LOCK
            .lock()
            .map_err(|_| "Botguard token generator lock is poisoned".to_string())?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("Could not start the YouTube token runtime: {e}"))?;

        runtime.block_on(async move {
            let mut botguard = rustypipe_botguard::Botguard::builder()
                .user_agent(WEB_USER_AGENT)
                .client(client)
                .snapshot_path(&snapshot_path)
                .init()
                .await
                .map_err(|e| format!("Could not initialize YouTube stream verification: {e}"))?;
            let player_token = botguard
                .mint_token(&video_id)
                .await
                .map_err(|e| format!("Could not mint YouTube player verification token: {e}"))?;
            let gvs_token = botguard
                .mint_token(&visitor_data)
                .await
                .map_err(|e| format!("Could not mint YouTube stream verification token: {e}"))?;
            if !botguard.is_from_snapshot() {
                let _ = botguard.write_snapshot().await;
            }
            Ok((gvs_token, player_token))
        })
    })
    .await
    .map_err(|e| format!("YouTube token generation task failed: {e}"))?
}

fn extract_config_string(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let mut offset = 0;
    while let Some(found) = text.get(offset..)?.find(&needle) {
        let key_end = offset + found + needle.len();
        let colon = text.get(key_end..)?.find(':')? + key_end + 1;
        let value_start = colon + text.get(colon..)?.len() - text.get(colon..)?.trim_start().len();
        if text.get(value_start..)?.starts_with('"') {
            let mut deserializer = serde_json::Deserializer::from_str(&text[value_start..]);
            if let Ok(value) = String::deserialize(&mut deserializer) {
                if !value.is_empty() {
                    return Some(value);
                }
            }
        }
        offset = key_end;
    }
    None
}

fn extract_config_number(text: &str, key: &str) -> Option<u64> {
    let needle = format!("\"{key}\"");
    let mut offset = 0;
    while let Some(found) = text.get(offset..)?.find(&needle) {
        let key_end = offset + found + needle.len();
        let colon = text.get(key_end..)?.find(':')? + key_end + 1;
        let value_start = colon + text.get(colon..)?.len() - text.get(colon..)?.trim_start().len();
        let mut deserializer = serde_json::Deserializer::from_str(&text[value_start..]);
        if let Ok(value) = u64::deserialize(&mut deserializer) {
            return Some(value);
        }
        offset = key_end;
    }
    None
}

/// Fetch the live Innertube identity from YouTube Music's public page config.
async fn fetch_innertube_config() -> Option<InnertubeConfig> {
    let client = build_client(WEB_USER_AGENT).ok()?;
    let resp = client.get("https://music.youtube.com/").send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let text = resp.text().await.ok()?;
    let (youtube_api_key, youtube_web_client_version, signature_timestamp) =
        match client.get("https://www.youtube.com/").send().await {
            Ok(response) if response.status().is_success() => response
                .text()
                .await
                .ok()
                .map(|html| {
                    (
                        extract_config_string(&html, "INNERTUBE_API_KEY"),
                        extract_config_string(&html, "INNERTUBE_CLIENT_VERSION"),
                        extract_config_number(&html, "STS")
                            .or_else(|| extract_config_number(&html, "signatureTimestamp")),
                    )
                })
                .unwrap_or((None, None, None)),
            _ => (None, None, None),
        };
    let youtube_api_key = youtube_api_key
        .or_else(|| extract_config_string(&text, "INNERTUBE_API_KEY"))
        .unwrap_or_else(|| FALLBACK_API_KEY.to_string());
    log::debug!(
        "YouTube player config loaded; live signature timestamp available: {}",
        signature_timestamp.is_some()
    );
    Some(InnertubeConfig {
        api_key: extract_config_string(&text, "INNERTUBE_API_KEY")
            .unwrap_or_else(|| FALLBACK_API_KEY.to_string()),
        youtube_api_key,
        client_version: extract_config_string(&text, "INNERTUBE_CLIENT_VERSION")
            .unwrap_or_else(|| FALLBACK_CLIENT_VERSION.to_string()),
        youtube_web_client_version: youtube_web_client_version
            .unwrap_or_else(|| FALLBACK_YOUTUBE_WEB_CLIENT_VERSION.to_string()),
        signature_timestamp,
        visitor_data: extract_config_string(&text, "VISITOR_DATA")
            .or_else(|| extract_config_string(&text, "visitorData")),
    })
}

async fn get_innertube_config() -> Result<InnertubeConfig, String> {
    if let Some(config) = INNERTUBE_CONFIG
        .lock()
        .map_err(|_| "Mutex poisoned")?
        .clone()
    {
        return Ok(config);
    }

    let config = fetch_innertube_config()
        .await
        .unwrap_or_else(|| InnertubeConfig {
            api_key: FALLBACK_API_KEY.to_string(),
            youtube_api_key: FALLBACK_API_KEY.to_string(),
            client_version: FALLBACK_CLIENT_VERSION.to_string(),
            youtube_web_client_version: FALLBACK_YOUTUBE_WEB_CLIENT_VERSION.to_string(),
            signature_timestamp: None,
            visitor_data: None,
        });
    if let Some(visitor_data) = config.visitor_data.as_ref() {
        *VISITOR_DATA.lock().map_err(|_| "Mutex poisoned")? = Some(visitor_data.clone());
    }
    *INNERTUBE_CONFIG.lock().map_err(|_| "Mutex poisoned")? = Some(config.clone());
    Ok(config)
}

async fn get_or_fetch_visitor_data() -> Result<Option<String>, String> {
    if let Some(visitor_data) = VISITOR_DATA.lock().map_err(|_| "Mutex poisoned")?.clone() {
        return Ok(Some(visitor_data));
    }
    Ok(get_innertube_config().await?.visitor_data)
}

fn remember_visitor_data(response: &Value) {
    if let Some(visitor_data) = response
        .pointer("/responseContext/visitorData")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        if let Ok(mut cached) = VISITOR_DATA.lock() {
            *cached = Some(visitor_data.to_string());
        }
    }
}

async fn post_json(
    client: &reqwest::Client,
    api_base: &str,
    endpoint: &str,
    body: Value,
    client_id: &str,
    client_version: &str,
    api_key: &str,
) -> Result<Value, String> {
    let url = format!("{api_base}/{endpoint}?prettyPrint=false&key={api_key}");

    let visitor_data = body
        .pointer("/context/client/visitorData")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty());
    let mut request = client
        .post(&url)
        .header("Origin", api_base.trim_end_matches("/youtubei/v1"))
        .header(
            "Referer",
            format!("{}/", api_base.trim_end_matches("/youtubei/v1")),
        )
        .header("X-YouTube-Client-Name", client_id)
        .header("X-YouTube-Client-Version", client_version)
        .header("X-Goog-Api-Key", api_key);
    if let Some(visitor_data) = visitor_data {
        request = request.header("X-Goog-Visitor-Id", visitor_data);
    }
    let resp = request
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Network request failed: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("YouTube Music returned HTTP {status}"));
    }

    let data = resp
        .json::<Value>()
        .await
        .map_err(|e| format!("Could not read YouTube Music response: {e}"))?;
    remember_visitor_data(&data);
    Ok(data)
}

fn parse_duration(s: &str) -> Option<u32> {
    let parts: Vec<&str> = s.trim().split(':').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return None;
    }
    let mut total = 0u32;
    for p in parts {
        total = total * 60 + p.parse::<u32>().ok()?;
    }
    Some(total)
}

fn secure_thumbnail_url(url: &str) -> Option<String> {
    let url = url.trim();
    if url.starts_with("//") {
        Some(format!("https:{url}"))
    } else if let Some(rest) = url.strip_prefix("http://") {
        Some(format!("https://{rest}"))
    } else if url.starts_with("https://") {
        Some(url.to_string())
    } else {
        None
    }
}

fn parse_item(item: &Value) -> Option<OnlineTrack> {
    let r = item.get("musicResponsiveListItemRenderer")?;
    let video_id = [
        "/playlistItemData/videoId",
        "/navigationEndpoint/watchEndpoint/videoId",
        "/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint/watchEndpoint/videoId",
        "/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint/watchPlaylistEndpoint/videoId",
    ]
    .iter()
    .find_map(|path| r.pointer(path).and_then(Value::as_str))
    .map(str::to_string)
    .or_else(|| {
        let playlist_id = r
            .pointer("/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint/watchPlaylistEndpoint/playlistId")
            .and_then(Value::as_str)?;
        let video_id = playlist_id.strip_prefix("RDAMVM")?;
        (video_id.len() == 11
            && video_id
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'))
        .then(|| video_id.to_string())
    })?;
    let title_column =
        r.pointer("/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text")?;
    let title = title_column
        .pointer("/runs/0/text")
        .and_then(Value::as_str)
        .or_else(|| title_column.get("simpleText").and_then(Value::as_str))?
        .trim()
        .to_string();
    if title.is_empty() {
        return None;
    }

    // Some YouTube Music layouts omit the metadata column or replace its
    // runs with simpleText. Metadata is useful but must not decide whether a
    // valid video ID/title can be shown or played.
    let metadata = r.pointer("/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text");
    let mut groups = Vec::new();
    if let Some(runs) = metadata
        .and_then(|value| value.get("runs"))
        .and_then(Value::as_array)
    {
        let mut current = String::new();
        for run in runs {
            let text = run.get("text").and_then(Value::as_str).unwrap_or("");
            if text.trim() == "\u{2022}" {
                if !current.trim().is_empty() {
                    groups.push(current.trim().to_string());
                }
                current.clear();
            } else {
                current.push_str(text);
            }
        }
        if !current.trim().is_empty() {
            groups.push(current.trim().to_string());
        }
    } else if let Some(simple_text) = metadata
        .and_then(|value| value.get("simpleText"))
        .and_then(Value::as_str)
    {
        groups.extend(
            simple_text
                .split('\u{2022}')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(str::to_string),
        );
    }
    groups.retain(|group| group != "Song" && group != "Video");

    let duration_secs = match groups.last().and_then(|g| parse_duration(g)) {
        Some(d) => {
            groups.pop();
            d
        }
        None => 0,
    };
    let artist = groups
        .first()
        .cloned()
        .unwrap_or_else(|| "Unknown artist".into());
    let album = groups.get(1).cloned();

    let thumbnail_url = [
        "/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails",
        "/thumbnail/thumbnailRenderer/thumbnail/thumbnails",
        "/thumbnail/thumbnails",
    ]
    .iter()
    .filter_map(|path| r.pointer(path).and_then(Value::as_array))
    .flat_map(|thumbnails| thumbnails.iter().rev())
    .filter_map(|thumbnail| thumbnail.get("url").and_then(Value::as_str))
    .find_map(secure_thumbnail_url)
    .or_else(|| Some(format!("https://i.ytimg.com/vi/{video_id}/hqdefault.jpg")));

    Some(OnlineTrack {
        video_id,
        title,
        artist,
        album,
        duration_secs,
        thumbnail_url,
    })
}

fn parse_video_item(item: &Value) -> Option<OnlineTrack> {
    let renderer = item.get("videoRenderer")?;
    let video_id = renderer.get("videoId")?.as_str()?.to_string();
    if video_id.len() != 11
        || !video_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return None;
    }
    let title = renderer
        .pointer("/title/runs/0/text")
        .and_then(Value::as_str)
        .or_else(|| {
            renderer
                .pointer("/title/simpleText")
                .and_then(Value::as_str)
        })?
        .to_string();
    let artist = [
        "/ownerText/runs/0/text",
        "/shortBylineText/runs/0/text",
        "/longBylineText/runs/0/text",
    ]
    .iter()
    .find_map(|path| renderer.pointer(path).and_then(Value::as_str))
    .unwrap_or("Unknown artist")
    .to_string();
    let duration_secs = renderer
        .pointer("/lengthText/simpleText")
        .and_then(Value::as_str)
        .and_then(parse_duration)
        .unwrap_or(0);
    let thumbnail_url = renderer
        .pointer("/thumbnail/thumbnails")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .rev()
        .filter_map(|thumbnail| thumbnail.get("url").and_then(Value::as_str))
        .find_map(secure_thumbnail_url)
        .or_else(|| Some(format!("https://i.ytimg.com/vi/{video_id}/hqdefault.jpg")));

    Some(OnlineTrack {
        video_id,
        title,
        artist,
        album: None,
        duration_secs,
        thumbnail_url,
    })
}

fn collect_tracks(
    value: &Value,
    tracks: &mut Vec<OnlineTrack>,
    seen: &mut HashSet<String>,
    limit: usize,
) {
    if tracks.len() >= limit {
        return;
    }

    match value {
        Value::Object(object) => {
            if object.contains_key("musicResponsiveListItemRenderer") {
                if let Some(track) = parse_item(value) {
                    if seen.insert(track.video_id.clone()) {
                        tracks.push(track);
                    }
                }
                return;
            }
            if object.contains_key("videoRenderer") {
                if let Some(track) = parse_video_item(value) {
                    if seen.insert(track.video_id.clone()) {
                        tracks.push(track);
                    }
                }
                return;
            }
            for child in object.values() {
                collect_tracks(child, tracks, seen, limit);
                if tracks.len() >= limit {
                    return;
                }
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_tracks(child, tracks, seen, limit);
                if tracks.len() >= limit {
                    return;
                }
            }
        }
        _ => {}
    }
}

fn parse_tracks(data: &Value, limit: usize) -> Vec<OnlineTrack> {
    let mut tracks = Vec::new();
    collect_tracks(data, &mut tracks, &mut HashSet::new(), limit);
    tracks
}

fn find_continuation_token(value: &Value) -> Option<String> {
    if let Value::Object(object) = value {
        for path in [
            "/continuationItemRenderer/continuationEndpoint/continuationCommand/token",
            "/continuations/0/nextContinuationData/continuation",
            "/continuations/0/nextRadioContinuationData/continuation",
        ] {
            if let Some(token) = value.pointer(path).and_then(Value::as_str) {
                return Some(token.to_string());
            }
        }
        for child in object.values() {
            if let Some(token) = find_continuation_token(child) {
                return Some(token);
            }
        }
    } else if let Value::Array(items) = value {
        for child in items {
            if let Some(token) = find_continuation_token(child) {
                return Some(token);
            }
        }
    }
    None
}

async fn fetch_search(
    query: &str,
    filter: Option<&str>,
    continuation: Option<&str>,
) -> Result<Value, String> {
    let client = build_client(WEB_USER_AGENT)?;
    let config = get_innertube_config().await?;

    let mut client_context = json!({
        "clientName": CLIENT_NAME,
        "clientVersion": config.client_version,
        "hl": "en",
        "gl": "US"
    });
    if let Some(visitor_data) = config.visitor_data.as_ref() {
        client_context["visitorData"] = json!(visitor_data);
    }

    let mut body = json!({ "context": { "client": client_context } });
    if let Some(continuation) = continuation {
        body["continuation"] = json!(continuation);
    } else {
        body["query"] = json!(query);
        if let Some(filter) = filter {
            body["params"] = json!(filter);
        }
    }

    post_json(
        &client,
        API_BASE,
        "search",
        body,
        CLIENT_ID,
        &config.client_version,
        &config.api_key,
    )
    .await
}

/// Search regular YouTube as a last-resort source. Music sometimes returns an
/// empty or changed catalog layout for valid queries; WEB search results still
/// carry playable video IDs that the player can resolve.
async fn fetch_youtube_web_search(query: &str) -> Result<Value, String> {
    let client = build_client(WEB_USER_AGENT)?;
    let config = get_innertube_config().await?;
    let mut client_context = json!({
        "clientName": YOUTUBE_WEB_CLIENT_NAME,
        "clientVersion": config.youtube_web_client_version,
        "hl": "en",
        "gl": "US"
    });
    if let Some(visitor_data) = config.visitor_data.as_ref() {
        client_context["visitorData"] = json!(visitor_data);
    }
    let body = json!({
        "context": {
            "client": client_context,
            "user": { "lockedSafetyMode": false }
        },
        "query": query
    });

    post_json(
        &client,
        YOUTUBE_API_BASE,
        "search",
        body,
        YOUTUBE_WEB_CLIENT_ID,
        &config.youtube_web_client_version,
        &config.youtube_api_key,
    )
    .await
}

async fn fetch_search_tracks(
    query: &str,
    filter: Option<&str>,
    limit: usize,
) -> Result<Vec<OnlineTrack>, String> {
    let mut page = fetch_search(query, filter, None).await?;
    let mut tracks = Vec::new();
    let mut seen = HashSet::new();
    let mut seen_continuations = HashSet::new();

    for page_number in 0..8 {
        for track in parse_tracks(&page, limit.saturating_sub(tracks.len())) {
            if seen.insert(track.video_id.clone()) {
                tracks.push(track);
                if tracks.len() >= limit {
                    return Ok(tracks);
                }
            }
        }

        if page_number == 7 || tracks.len() >= limit {
            break;
        }
        let Some(token) = find_continuation_token(&page) else {
            break;
        };
        if !seen_continuations.insert(token.clone()) {
            break;
        }
        page = fetch_search(query, None, Some(&token)).await?;
    }
    Ok(tracks)
}

const SOUNDCLOUD_HOME: &str = "https://soundcloud.com/";
const SOUNDCLOUD_API: &str = "https://api-v2.soundcloud.com";
const SOUNDCLOUD_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

fn cached_soundcloud_client_id() -> Option<String> {
    SOUNDCLOUD_CLIENT_ID.lock().ok().and_then(|id| id.clone())
}

fn extract_soundcloud_client_id(source: &str) -> Option<String> {
    let mut offset = 0;
    while let Some(found) = source.get(offset..)?.find("client_id") {
        let after_key = offset + found + "client_id".len();
        let mut rest = source.get(after_key..)?;
        let whitespace = rest.len() - rest.trim_start().len();
        rest = &rest[whitespace..];
        if let Some(key_quote) = rest.chars().next().filter(|ch| *ch == '"' || *ch == '\'') {
            let quoted_key = &rest[1..];
            rest = quoted_key;
            if let Some(end_quote) = rest.find(key_quote) {
                rest = &rest[end_quote + 1..];
            } else {
                offset = after_key;
                continue;
            }
            rest = rest.trim_start();
        }
        if let Some(value) = rest.strip_prefix(':').or_else(|| rest.strip_prefix('=')) {
            let value = value.trim_start();
            let Some(quote) = value.chars().next().filter(|ch| *ch == '"' || *ch == '\'') else {
                offset = after_key;
                continue;
            };
            if quote == '"' || quote == '\'' {
                let candidate = value.get(1..)?.get(..32)?;
                if candidate.chars().all(|ch| ch.is_ascii_alphanumeric())
                    && value.get(33..)?.starts_with(quote)
                {
                    return Some(candidate.to_owned());
                }
            }
        }
        offset = after_key;
    }
    None
}

fn soundcloud_script_sources(page: &str) -> Vec<String> {
    let mut sources = Vec::new();
    let mut offset = 0;
    while let Some(found) = page.get(offset..).and_then(|tail| tail.find("<script")) {
        let start = offset + found;
        let Some(tag_end) = page.get(start..).and_then(|tail| tail.find('>')) else {
            break;
        };
        let end = start + tag_end;
        let tag = &page[start..end];
        if let Some(src_pos) = tag.find("src=") {
            let src = tag[src_pos + 4..].trim_start();
            if let Some(quote) = src.chars().next().filter(|ch| *ch == '\'' || *ch == '"') {
                if let Some(value) = src.get(1..).and_then(|tail| tail.find(quote).map(|len| &tail[..len])) {
                    let url = if value.starts_with("//") {
                        format!("https:{value}")
                    } else if value.starts_with('/') {
                        format!("https://soundcloud.com{value}")
                    } else {
                        value.to_owned()
                    };
                    if url.starts_with("https://") {
                        sources.push(url);
                    }
                }
            }
        }
        offset = end + 1;
        if sources.len() >= 40 {
            break;
        }
    }
    sources
}

async fn get_soundcloud_client_id(client: &reqwest::Client) -> Result<String, String> {
    if let Some(id) = cached_soundcloud_client_id() {
        return Ok(id);
    }

    let page = client
        .get(SOUNDCLOUD_HOME)
        .header("Accept", "text/html,application/xhtml+xml")
        .send()
        .await
        .map_err(|e| format!("Could not reach SoundCloud: {e}"))?;
    if !page.status().is_success() {
        return Err(format!("SoundCloud home page returned HTTP {}", page.status()));
    }
    let html = page
        .text()
        .await
        .map_err(|e| format!("Could not read SoundCloud's home page: {e}"))?;
    if let Some(id) = extract_soundcloud_client_id(&html) {
        if let Ok(mut cached) = SOUNDCLOUD_CLIENT_ID.lock() {
            *cached = Some(id.clone());
        }
        return Ok(id);
    }

    // SoundCloud's web app keeps its public client ID in the bundled scripts.
    // Refreshing from the current page avoids requiring a user-supplied key.
    for script_url in soundcloud_script_sources(&html).into_iter().rev().take(24) {
        let response = match client
            .get(script_url)
            .header("Referer", SOUNDCLOUD_HOME)
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => response,
            _ => continue,
        };
        let script = match response.text().await {
            Ok(script) => script,
            Err(_) => continue,
        };
        if let Some(id) = extract_soundcloud_client_id(&script) {
            if let Ok(mut cached) = SOUNDCLOUD_CLIENT_ID.lock() {
                *cached = Some(id.clone());
            }
            return Ok(id);
        }
    }
    Err("Could not find SoundCloud's public web client ID".into())
}

fn clear_soundcloud_client_id(id: &str) {
    if let Ok(mut cached) = SOUNDCLOUD_CLIENT_ID.lock() {
        if cached.as_deref() == Some(id) {
            *cached = None;
        }
    }
}

async fn soundcloud_api_json(
    client: &reqwest::Client,
    endpoint: &str,
    params: &[(&str, String)],
) -> Result<Value, String> {
    for attempt in 0..2 {
        let client_id = get_soundcloud_client_id(client).await?;
        let mut url = reqwest::Url::parse(endpoint)
            .map_err(|e| format!("Invalid SoundCloud API URL: {e}"))?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("client_id", &client_id);
            for (name, value) in params {
                query.append_pair(name, value);
            }
        }
        let response = client
            .get(url)
            .header("Origin", "https://soundcloud.com")
            .header("Referer", SOUNDCLOUD_HOME)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| format!("SoundCloud request failed: {e}"))?;
        let status = response.status();
        if (status == reqwest::StatusCode::UNAUTHORIZED
            || status == reqwest::StatusCode::FORBIDDEN)
            && attempt == 0
        {
            clear_soundcloud_client_id(&client_id);
            continue;
        }
        if !status.is_success() {
            let reason = if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                "SoundCloud rate limit reached".to_owned()
            } else {
                format!("SoundCloud returned HTTP {status}")
            };
            return Err(reason);
        }
        return response
            .json::<Value>()
            .await
            .map_err(|e| format!("Could not parse SoundCloud response: {e}"));
    }
    Err("SoundCloud rejected its current web client ID".into())
}

fn soundcloud_artwork_url(value: Option<&str>) -> Option<String> {
    let url = value?.trim();
    if !url.starts_with("https://") {
        return None;
    }
    let path_end = url.find(|ch| ch == '?' || ch == '#').unwrap_or(url.len());
    let path = url.get(..path_end)?;
    let extension = path.rfind('.')?;
    let stem = path.get(..extension)?;
    let extension = path.get(extension..)?;
    let suffix_start = stem.rfind('-')? + 1;
    let suffix = stem.get(suffix_start..)?;
    if matches!(suffix, "large" | "t300x300" | "t500x500" | "crop" | "small") {
        Some(format!("{}-t500x500{}", &stem[..suffix_start - 1], extension))
    } else {
        Some(url.to_owned())
    }
}

fn parse_soundcloud_track(item: &Value) -> Option<OnlineTrack> {
    if item.get("streamable").and_then(Value::as_bool) == Some(false)
        || item.pointer("/policy").and_then(Value::as_str) == Some("BLOCK")
    {
        return None;
    }
    let id = item
        .get("id")
        .and_then(Value::as_u64)
        .map(|id| id.to_string())
        .or_else(|| item.get("id").and_then(Value::as_str).map(str::to_owned))?;
    if id.is_empty() || !id.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let title = item.get("title").and_then(Value::as_str)?.trim();
    if title.is_empty() {
        return None;
    }
    let artist = item
        .pointer("/publisher_metadata/artist")
        .and_then(Value::as_str)
        .or_else(|| item.pointer("/user/username").and_then(Value::as_str))
        .unwrap_or("Unknown artist")
        .to_owned();
    let album = item
        .pointer("/publisher_metadata/album_title")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let duration_secs = item
        .get("duration")
        .and_then(Value::as_u64)
        .map(|ms| (ms / 1000).min(u64::from(u32::MAX)) as u32)
        .unwrap_or(0);
    let thumbnail_url = soundcloud_artwork_url(
        item.get("artwork_url")
            .and_then(Value::as_str)
            .or_else(|| item.pointer("/user/avatar_url").and_then(Value::as_str)),
    );
    Some(OnlineTrack {
        video_id: id,
        title: title.to_owned(),
        artist,
        album,
        duration_secs,
        thumbnail_url,
    })
}

async fn search_soundcloud_tracks(query: &str, limit: usize) -> Result<Vec<OnlineTrack>, String> {
    let client = build_client_with_timeout(SOUNDCLOUD_USER_AGENT, Duration::from_secs(20))?;
    let response = soundcloud_api_json(
        &client,
        &format!("{SOUNDCLOUD_API}/search/tracks"),
        &[("q", query.to_owned()), ("limit", limit.to_string())],
    )
    .await?;
    let collection = response
        .get("collection")
        .and_then(Value::as_array)
        .ok_or("SoundCloud returned no track collection")?;
    let mut seen = HashSet::new();
    Ok(collection
        .iter()
        .filter_map(parse_soundcloud_track)
        .filter(|track| seen.insert(track.video_id.clone()))
        .take(limit)
        .collect())
}

fn soundcloud_mime_extension(mime: &str) -> Option<&'static str> {
    let mime = mime.split(';').next().unwrap_or_default().trim().to_ascii_lowercase();
    match mime.as_str() {
        "audio/mpeg" | "audio/mp3" => Some("mp3"),
        "audio/aac" | "audio/x-aac" => Some("aac"),
        "audio/mp4" | "audio/x-m4a" => Some("m4a"),
        "audio/ogg" | "application/ogg" => Some("ogg"),
        "audio/webm" => Some("webm"),
        "audio/opus" => Some("opus"),
        _ => None,
    }
}

async fn resolve_soundcloud_media_url(track_id: &str) -> Result<(String, String, bool), String> {
    if track_id.is_empty() || !track_id.chars().all(|ch| ch.is_ascii_digit()) {
        return Err("Invalid SoundCloud track ID".into());
    }
    let client = build_client_with_timeout(SOUNDCLOUD_USER_AGENT, Duration::from_secs(30))?;
    let track = soundcloud_api_json(
        &client,
        &format!("{SOUNDCLOUD_API}/tracks/{track_id}"),
        &[],
    )
    .await?;
    let transcodings = track
        .pointer("/media/transcodings")
        .and_then(Value::as_array)
        .ok_or("SoundCloud did not provide playable audio formats")?;
    let mut choices: Vec<&Value> = transcodings
        .iter()
        .filter(|item| item.get("snipped").and_then(Value::as_bool) != Some(true))
        .filter(|item| item.get("url").and_then(Value::as_str).is_some())
        .filter(|item| {
            item.pointer("/format/mime_type")
                .and_then(Value::as_str)
                .and_then(soundcloud_mime_extension)
                .is_some()
        })
        .collect();
    choices.sort_by_key(|item| {
        let protocol = item.pointer("/format/protocol").and_then(Value::as_str).unwrap_or("");
        let mime = item.pointer("/format/mime_type").and_then(Value::as_str).unwrap_or("");
        let protocol_rank = if protocol == "progressive" { 0 } else if protocol == "hls" { 1 } else { 2 };
        let codec_rank = if mime.contains("mpeg") { 0 } else if mime.contains("mp4") || mime.contains("aac") { 1 } else { 2 };
        (protocol_rank, codec_rank)
    });
    let choice = choices.first().ok_or("SoundCloud has no supported audio format for this track")?;
    let transcoding_url = choice.get("url").and_then(Value::as_str).unwrap_or_default();
    let resolved = soundcloud_api_json(&client, transcoding_url, &[]).await?;
    let media_url = resolved
        .get("url")
        .and_then(Value::as_str)
        .ok_or("SoundCloud did not return a playable stream URL")?
        .to_owned();
    let mime = choice
        .pointer("/format/mime_type")
        .and_then(Value::as_str)
        .unwrap_or("audio/mpeg")
        .to_owned();
    let is_hls = choice.pointer("/format/protocol").and_then(Value::as_str) == Some("hls");
    Ok((media_url, mime, is_hls))
}

async fn fetch_soundcloud_bytes(client: &reqwest::Client, url: &str) -> Result<(Vec<u8>, reqwest::Url), String> {
    const MAX_AUDIO_BYTES: usize = 100 * 1024 * 1024;
    let response = client
        .get(url)
        .header("Referer", SOUNDCLOUD_HOME)
        .send()
        .await
        .map_err(|e| format!("SoundCloud stream request failed: {e}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("SoundCloud stream returned HTTP {status}"));
    }
    let final_url = response.url().clone();
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Could not read SoundCloud audio: {e}"))?;
    if bytes.len() > MAX_AUDIO_BYTES {
        return Err("SoundCloud audio exceeds the 100 MB playback limit".into());
    }
    Ok((bytes.to_vec(), final_url))
}

fn manifest_attribute<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let start = line.find(key)? + key.len();
    let value = line.get(start..)?.strip_prefix('"')?;
    let end = value.find('"')?;
    value.get(..end)
}

async fn download_soundcloud_hls(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, String> {
    const MAX_AUDIO_BYTES: usize = 100 * 1024 * 1024;
    let mut playlist_url = reqwest::Url::parse(url).map_err(|e| format!("Invalid HLS URL: {e}"))?;
    for _ in 0..4 {
        let (bytes, final_url) = fetch_soundcloud_bytes(client, playlist_url.as_str()).await?;
        let text = String::from_utf8_lossy(&bytes);
        if !text.trim_start().starts_with("#EXTM3U") {
            return Ok(bytes);
        }
        let lines: Vec<&str> = text.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
        let variant = lines
            .iter()
            .enumerate()
            .filter_map(|(index, line)| {
                if !line.starts_with("#EXT-X-STREAM-INF:") {
                    return None;
                }
                let bandwidth = line
                    .split("BANDWIDTH=")
                    .nth(1)
                    .and_then(|value| value.split(|ch: char| !ch.is_ascii_digit()).next())
                    .and_then(|value| value.parse::<u64>().ok())
                    .unwrap_or(0);
                lines.get(index + 1).copied().map(|uri| (bandwidth, uri))
            })
            .max_by_key(|(bandwidth, _)| *bandwidth)
            .map(|(_, uri)| uri);
        if let Some(variant) = variant {
            playlist_url = final_url
                .join(variant)
                .map_err(|e| format!("Invalid SoundCloud HLS variant URL: {e}"))?;
            continue;
        }
        if lines.iter().any(|line| line.starts_with("#EXT-X-KEY:") && !line.contains("METHOD=NONE")) {
            return Err("SoundCloud returned an encrypted HLS stream that Pulse cannot decode".into());
        }

        let map_url = lines
            .iter()
            .find(|line| line.starts_with("#EXT-X-MAP:"))
            .and_then(|line| manifest_attribute(line, "URI="))
            .and_then(|uri| final_url.join(uri).ok());
        let mut audio = Vec::new();
        if let Some(map_url) = map_url {
            let (map, _) = fetch_soundcloud_bytes(client, map_url.as_str()).await?;
            audio.extend_from_slice(&map);
        }
        let segments: Vec<String> = lines
            .iter()
            .filter(|line| !line.starts_with('#'))
            .map(|line| {
                final_url
                    .join(line)
                    .map(|url| url.to_string())
                    .map_err(|e| format!("Invalid SoundCloud HLS segment URL: {e}"))
            })
            .collect::<Result<_, _>>()?;
        if segments.is_empty() {
            return Err("SoundCloud returned an empty HLS playlist".into());
        }
        for segment in segments {
            let (bytes, _) = fetch_soundcloud_bytes(client, &segment).await?;
            if audio.len().saturating_add(bytes.len()) > MAX_AUDIO_BYTES {
                return Err("SoundCloud audio exceeds the 100 MB playback limit".into());
            }
            audio.extend_from_slice(&bytes);
        }
        return Ok(audio);
    }
    Err("SoundCloud HLS playlist has too many nested variants".into())
}

async fn download_soundcloud_track(track_id: &str) -> Result<PathBuf, String> {
    const MAX_AUDIO_BYTES: usize = 100 * 1024 * 1024;
    const STREAM_TIMEOUT: Duration = Duration::from_secs(180);
    let cache_dir = std::env::temp_dir().join("pulse-online-cache");
    fs::create_dir_all(&cache_dir).map_err(|e| format!("Could not create the online cache: {e}"))?;
    for extension in ["mp3", "aac", "m4a", "ogg", "webm", "opus"] {
        let cached = cache_dir.join(format!("{track_id}.soundcloud.{extension}"));
        if fs::metadata(&cached).map(|metadata| metadata.len() > 1024).unwrap_or(false) {
            return Ok(cached);
        }
    }
    let client = build_client_with_timeout(SOUNDCLOUD_USER_AGENT, STREAM_TIMEOUT)?;
    let (url, mime, is_hls) = resolve_soundcloud_media_url(track_id).await?;
    let extension = soundcloud_mime_extension(&mime).ok_or("SoundCloud returned an unsupported audio format")?;
    let audio = tokio::time::timeout(STREAM_TIMEOUT, async {
        if is_hls {
            download_soundcloud_hls(&client, &url).await
        } else {
            fetch_soundcloud_bytes(&client, &url).await.map(|(bytes, _)| bytes)
        }
    })
    .await
    .map_err(|_| "SoundCloud audio download timed out".to_string())??;
    if audio.len() < 1024 {
        return Err("SoundCloud ended the audio stream before sending playable audio".into());
    }
    if audio.len() > MAX_AUDIO_BYTES {
        return Err("SoundCloud audio exceeds the 100 MB playback limit".into());
    }
    let path = cache_dir.join(format!("{track_id}.soundcloud.{extension}"));
    let partial = cache_dir.join(format!("{track_id}.soundcloud.{extension}.part"));
    fs::write(&partial, &audio).map_err(|e| format!("Could not cache SoundCloud audio: {e}"))?;
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Could not replace cached SoundCloud audio: {e}"))?;
    }
    fs::rename(&partial, &path).map_err(|e| format!("Could not finalize SoundCloud audio: {e}"))?;
    prune_audio_cache(&cache_dir, &path);
    Ok(path)
}

pub async fn search_tracks(query: &str, limit: usize) -> Result<Vec<OnlineTrack>, String> {
    let query = query.trim();
    if query.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    search_soundcloud_tracks(query, limit.min(50)).await
}

async fn fetch_player(
    video_id: &str,
    client_name: &str,
    client_id: &str,
    client_version: &str,
    api_base: &str,
    user_agent: &str,
) -> Result<Value, String> {
    let client = build_client(user_agent)?;
    let config = get_innertube_config().await?;
    let mut client_context = json!({
        "clientName": client_name,
        "clientVersion": client_version,
        "hl": "en",
        "gl": "US",
        "timeZone": "UTC",
        "utcOffsetMinutes": 0,
        "clientScreen": "WATCH"
    });
    if client_name == YOUTUBE_EMBEDDED_CLIENT_NAME {
        client_context["userAgent"] = json!(EMBEDDED_USER_AGENT);
    }
    if let Some(visitor_data) = get_or_fetch_visitor_data().await? {
        client_context["visitorData"] = json!(visitor_data);
    }
    let mut context = json!({ "client": client_context });
    if client_name == YOUTUBE_EMBEDDED_CLIENT_NAME {
        context["thirdParty"] = json!({ "embedUrl": "https://www.reddit.com/" });
    }
    let mut content_playback_context = json!({
        "html5Preference": "HTML5_PREF_WANTS"
    });
    if let Some(signature_timestamp) = config.signature_timestamp {
        content_playback_context["signatureTimestamp"] = json!(signature_timestamp);
    }
    let body = json!({
        "context": context,
        "videoId": video_id,
        "playbackContext": {
            "contentPlaybackContext": content_playback_context
        },
        "contentCheckOk": true,
        "racyCheckOk": true
    });
    let api_key = if api_base == YOUTUBE_API_BASE {
        &config.youtube_api_key
    } else {
        &config.api_key
    };
    post_json(
        &client,
        api_base,
        "player",
        body,
        client_id,
        client_version,
        api_key,
    )
    .await
}

fn select_audio_stream(data: &Value) -> Result<(String, String), String> {
    if let Some(status) = data
        .pointer("/playabilityStatus/status")
        .and_then(Value::as_str)
    {
        if status != "OK" {
            let reason = data
                .pointer("/playabilityStatus/reason")
                .and_then(Value::as_str)
                .unwrap_or("not playable");
            return Err(format!("Track cannot be played: {reason}"));
        }
    }
    let formats = data
        .pointer("/streamingData/adaptiveFormats")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "YouTube returned no direct audio formats for this player client".to_string()
        })?;

    let streaming_fields = data
        .get("streamingData")
        .and_then(Value::as_object)
        .map(|streaming| streaming.keys().cloned().collect::<Vec<_>>().join(","))
        .unwrap_or_default();
    log::debug!("YouTube streamingData fields: {streaming_fields}");
    log::debug!(
        "YouTube player formats: {}",
        formats
            .iter()
            .filter_map(|format| {
                let mime = format.get("mimeType")?.as_str()?;
                let itag = format.get("itag").and_then(Value::as_u64).unwrap_or(0);
                let direct = format.get("url").and_then(Value::as_str).is_some();
                let cipher = format
                    .get("signatureCipher")
                    .or_else(|| format.get("cipher"))
                    .and_then(Value::as_str)
                    .is_some();
                let fields = format
                    .as_object()
                    .map(|object| object.keys().cloned().collect::<Vec<_>>().join("+"))
                    .unwrap_or_default();
                Some(format!("{mime} itag={itag} direct={direct} cipher={cipher} fields={fields}"))
            })
            .collect::<Vec<_>>()
            .join(", ")
    );

    // Prefer broadly compatible streams, then use WebM/Opus where YouTube does
    // not expose a supported AAC or MP3 stream. The audio engine registers the
    // bundled libopus adapter and can decode these WebM streams directly.
    let mut audio: Vec<(u8, u64, String, String)> = formats
        .iter()
        .filter_map(|f| {
            let mime = f.get("mimeType")?.as_str()?;
            let mime_type = mime.split(';').next()?.trim().to_ascii_lowercase();
            let rank = match mime_type.as_str() {
                "audio/mp4" => 0,
                "audio/aac" => 1,
                "audio/mpeg" => 2,
                "audio/ogg" => 3,
                "audio/webm" => 4,
                "audio/opus" => 5,
                _ => return None,
            };
            let url = f.get("url")?.as_str()?;
            let bitrate = f.get("bitrate").and_then(Value::as_u64).unwrap_or(0);
            Some((rank, bitrate, url.to_string(), mime_type))
        })
        .collect();

    if audio.is_empty() {
        return Err(
            "YouTube did not return a directly playable AAC, MP3, Vorbis, or Opus stream for this track"
                .into(),
        );
    }
    audio.sort_by_key(|(rank, bitrate, _, _)| (*rank, std::cmp::Reverse(*bitrate)));
    let (_, _, url, mime) = audio.remove(0);
    Ok((url, mime))
}

async fn resolve_audio_stream_once(video_id: &str) -> Result<(String, String), String> {
    let config = get_innertube_config().await?;
    let clients = [
        (
            YOUTUBE_WEB_CLIENT_NAME,
            YOUTUBE_WEB_CLIENT_ID,
            config.youtube_web_client_version.as_str(),
            YOUTUBE_API_BASE,
            WEB_USER_AGENT,
        ),
        (
            YOUTUBE_EMBEDDED_CLIENT_NAME,
            YOUTUBE_EMBEDDED_CLIENT_ID,
            config.youtube_web_client_version.as_str(),
            YOUTUBE_API_BASE,
            EMBEDDED_USER_AGENT,
        ),
        (
            CLIENT_NAME,
            CLIENT_ID,
            config.client_version.as_str(),
            API_BASE,
            WEB_USER_AGENT,
        ),
    ];
    let mut last_error = "YouTube did not return a playable stream".to_string();
    let mut bot_challenge = None;

    for (client_name, client_id, client_version, api_base, user_agent) in clients {
        match fetch_player(
            video_id,
            client_name,
            client_id,
            client_version,
            api_base,
            user_agent,
        )
        .await
        {
            Ok(data) => {
                match select_audio_stream(&data) {
                    Ok(stream) => return Ok(stream),
                    Err(error) => {
                        log::debug!("YouTube player client {client_name} could not play {video_id}: {error}");
                        if error.to_ascii_lowercase().contains("not a bot") {
                            bot_challenge = Some(error.clone());
                        }
                        last_error = error;
                    }
                }
            }
            Err(error) => {
                log::debug!(
                    "YouTube player client {client_name} request failed for {video_id}: {error}"
                );
                if error.to_ascii_lowercase().contains("not a bot") {
                    bot_challenge = Some(error.clone());
                }
                last_error = error;
            }
        }
    }
    Err(bot_challenge.unwrap_or(last_error))
}

async fn resolve_audio_stream(video_id: &str) -> Result<(String, String), String> {
    if video_id.is_empty()
        || !video_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("Invalid video id".into());
    }

    match resolve_audio_stream_once(video_id).await {
        Err(error) if error.to_ascii_lowercase().contains("not a bot") => {
            log::debug!("YouTube challenged the guest player session; refreshing it once");
            reset_youtube_guest_session();
            resolve_audio_stream_once(video_id).await
        }
        result => result,
    }
}

pub async fn resolve_stream_url(video_id: &str) -> Result<String, String> {
    resolve_audio_stream(video_id).await.map(|(url, _)| url)
}

fn stream_extension(mime: &str) -> Option<&'static str> {
    let mime = mime
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    match mime.as_str() {
        "audio/mp4" => Some("m4a"),
        "audio/aac" => Some("aac"),
        "audio/mpeg" => Some("mp3"),
        "audio/ogg" => Some("ogg"),
        "audio/webm" => Some("webm"),
        "audio/opus" => Some("opus"),
        _ => None,
    }
}

/// Fetch audio through YouTube's SABR protocol when the player response has no
/// direct media URL. The same configured client preserves the user's proxy and
/// guest-cookie settings used by search and direct downloads.
async fn download_sabr_stream(video_id: &str) -> Result<PathBuf, String> {
    const MAX_AUDIO_BYTES: usize = 100 * 1024 * 1024;
    const SABR_TIMEOUT: Duration = Duration::from_secs(180);

    let cache_dir = std::env::temp_dir().join("pulse-online-cache");
    fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("Could not create the online music cache: {e}"))?;
    for extension in ["webm", "m4a", "aac", "mp3", "ogg", "opus"] {
        let cached_path = cache_dir.join(format!("{video_id}.sabr.{extension}"));
        if fs::metadata(&cached_path)
            .map(|metadata| metadata.len() > 1024)
            .unwrap_or(false)
        {
            return Ok(cached_path);
        }
    }

    let config = get_innertube_config().await?;
    let visitor_data = get_or_fetch_visitor_data()
        .await?
        .filter(|visitor| !visitor.is_empty())
        .ok_or_else(|| "YouTube did not provide the visitor ID required for audio playback".to_string())?;
    let (gvs_po_token, _player_po_token) =
        mint_playback_po_tokens(video_id.to_string(), visitor_data.clone()).await?;
    let client = build_client_with_timeout(WEB_USER_AGENT, Duration::from_secs(30))?;
    let (info, mut chunks) = sabr_rs::stream_audio(
        &client,
        video_id,
        &visitor_data,
        &gvs_po_token,
        &config.youtube_api_key,
        &config.youtube_web_client_version,
        config.signature_timestamp,
    )
    .await
    .map_err(|e| format!("Could not start YouTube's audio stream: {e}"))?;
    log::debug!("YouTube SABR selected {} for {video_id}", info.mime_type);
    let extension = stream_extension(&info.mime_type)
        .ok_or_else(|| format!("YouTube returned an unsupported audio format: {}", info.mime_type))?;
    let path = cache_dir.join(format!("{video_id}.sabr.{extension}"));

    let audio = tokio::time::timeout(SABR_TIMEOUT, async {
        let mut audio = Vec::new();
        let mut chunk_count = 0usize;
        while let Some(chunk) = chunks.recv().await {
            let chunk = chunk.map_err(|error| format!("YouTube SABR stream failed: {error}"))?;
            if audio.len().saturating_add(chunk.len()) > MAX_AUDIO_BYTES {
                return Err("The selected audio stream is unexpectedly large".to_string());
            }
            audio.extend_from_slice(&chunk);
            chunk_count += 1;
            if chunk_count == 1 {
                log::debug!("YouTube SABR delivered the first audio chunk for {video_id}");
            }
        }
        log::debug!("YouTube SABR ended after {chunk_count} chunk(s), {} bytes", audio.len());
        Ok::<_, String>(audio)
    })
    .await
    .map_err(|_| "YouTube audio transfer timed out".to_string())??;

    if audio.len() < 1024 {
        return Err("YouTube ended the audio stream before sending playable audio".into());
    }

    let partial_path = cache_dir.join(format!("{video_id}.sabr.{extension}.part"));
    fs::write(&partial_path, audio)
        .map_err(|e| format!("Could not cache YouTube audio: {e}"))?;
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Could not replace cached audio: {e}"))?;
    }
    fs::rename(&partial_path, &path)
        .map_err(|e| format!("Could not finalize cached audio: {e}"))?;
    prune_audio_cache(&cache_dir, &path);
    Ok(path)
}

async fn download_stream(video_id: &str, url: &str, mime: &str) -> Result<PathBuf, String> {
    let extension = stream_extension(mime)
        .ok_or_else(|| format!("Unsupported YouTube audio format: {mime}"))?;
    let cache_dir = std::env::temp_dir().join("pulse-online-cache");
    fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("Could not create the online music cache: {e}"))?;
    let path = cache_dir.join(format!("{video_id}.{extension}"));

    if fs::metadata(&path)
        .map(|metadata| metadata.len() > 1024)
        .unwrap_or(false)
    {
        return Ok(path);
    }

    let client = build_client_with_timeout(WEB_USER_AGENT, Duration::from_secs(120))?;
    let response = client
        .get(url)
        .header("Referer", "https://www.youtube.com/")
        .send()
        .await
        .map_err(|e| format!("Could not download YouTube audio: {e}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("YouTube audio server returned HTTP {status}"));
    }
    if response
        .content_length()
        .map(|size| size > 100 * 1024 * 1024)
        .unwrap_or(false)
    {
        return Err("The selected audio stream is unexpectedly large".into());
    }
    if response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_ascii_lowercase().starts_with("text/html"))
        .unwrap_or(false)
    {
        return Err("YouTube returned a web page instead of an audio stream".into());
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Could not read YouTube audio: {e}"))?;
    if bytes.len() < 1024 {
        return Err("YouTube returned an empty or incomplete audio stream".into());
    }
    if bytes.len() > 100 * 1024 * 1024 {
        return Err("The selected audio stream is unexpectedly large".into());
    }
    let partial_path = cache_dir.join(format!("{video_id}.{extension}.part"));
    fs::write(&partial_path, &bytes).map_err(|e| format!("Could not cache YouTube audio: {e}"))?;
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Could not replace cached audio: {e}"))?;
    }
    fs::rename(&partial_path, &path)
        .map_err(|e| format!("Could not finalize cached audio: {e}"))?;
    prune_audio_cache(&cache_dir, &path);
    Ok(path)
}

fn prune_audio_cache(cache_dir: &Path, keep: &Path) {
    const MAX_CACHE_FILES: usize = 50;
    const MAX_CACHE_BYTES: u64 = 512 * 1024 * 1024;

    let Ok(entries) = fs::read_dir(cache_dir) else {
        return;
    };
    let mut files: Vec<(PathBuf, u64, std::time::SystemTime)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() || path == keep {
                return None;
            }
            Some((
                path,
                metadata.len(),
                metadata.modified().unwrap_or(std::time::UNIX_EPOCH),
            ))
        })
        .collect();
    files.sort_by_key(|(_, _, modified)| *modified);

    let mut total_bytes: u64 = files.iter().map(|(_, size, _)| *size).sum();
    let mut file_count = files.len() + 1;
    for (path, size, _) in files {
        if file_count <= MAX_CACHE_FILES && total_bytes <= MAX_CACHE_BYTES {
            break;
        }
        if fs::remove_file(path).is_ok() {
            file_count = file_count.saturating_sub(1);
            total_bytes = total_bytes.saturating_sub(size);
        }
    }
}

#[tauri::command]
pub async fn online_play(state: State<'_, AppState>, track: OnlineTrack) -> Result<(), String> {
    if track.video_id.is_empty()
        || !track
            .video_id
            .chars()
            .all(|c| c.is_ascii_digit())
    {
        return Err("Invalid SoundCloud track ID".into());
    }

    let path = download_soundcloud_track(&track.video_id).await?;
    let path_string = path.to_string_lossy().into_owned();
    let filename = path
        .file_name()
        .unwrap_or_else(|| Path::new("online-track").as_os_str())
        .to_string_lossy()
        .into_owned();
    let title = if track.title.trim().is_empty() {
        "Unknown title".to_string()
    } else {
        track.title
    };
    let artist = if track.artist.trim().is_empty() {
        "Unknown artist".to_string()
    } else {
        track.artist
    };
    let audio_track = Track {
        id: -1,
        folder_id: None,
        path: path_string,
        filename,
        title,
        artist: artist.clone(),
        album: track.album.unwrap_or_default(),
        album_artist: artist,
        genre: "".into(),
        year: None,
        track_no: None,
        duration_ms: i64::from(track.duration_secs).saturating_mul(1000),
        bitrate_kbps: None,
        sample_rate: None,
        art_path: track.thumbnail_url,
        play_count: 0,
        added_at: 0,
        rating: 0,
        is_favorite: false,
        last_played: None,
        track_gain_db: None,
        track_peak: None,
    };
    state.audio.send(AudioCmd::PlayDirectTrack {
        track: audio_track,
        offset_ms: 0,
    });
    Ok(())
}

#[tauri::command]
pub async fn online_search(
    query: String,
    limit: Option<usize>,
) -> Result<Vec<OnlineTrack>, String> {
    let tracks = search_tracks(&query, limit.unwrap_or(50).min(50)).await?;
    log::debug!(
        "online_search returned {} track(s) for {:?}",
        tracks.len(),
        query
    );
    Ok(tracks)
}

#[tauri::command]
pub async fn online_resolve(video_id: String) -> Result<String, String> {
    resolve_soundcloud_media_url(&video_id)
        .await
        .map(|(url, _, _)| url)
}

#[tauri::command]
pub fn online_set_proxy(enabled: bool) {
    let previous = USE_SYSTEM_PROXY.swap(enabled, Ordering::AcqRel);
    if previous != enabled {
        if let Ok(mut client_id) = SOUNDCLOUD_CLIENT_ID.lock() {
            *client_id = None;
        }
        log::info!(
            "SoundCloud system proxy {}",
            if enabled { "enabled" } else { "disabled" }
        );
    }
}
