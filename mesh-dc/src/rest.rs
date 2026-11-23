//! Snapshot building and the REST calls, with the headers the web client sends
//! (token in `Authorization`, the super-properties, a browser user-agent). No
//! bot endpoints and no polling: messages are fetched once on open, then the
//! gateway keeps them current.

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use crate::gateway::{self, State};

/// A REST client carrying the account's auth headers.
#[derive(Clone)]
pub struct Rest {
    http: reqwest::Client,
    token: String,
}

impl Rest {
    pub fn new(token: String) -> Self {
        // Without timeouts one stalled request (the VPN dropping mid-call) hangs its caller for good.
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .unwrap_or_default();
        Rest { http, token }
    }

    fn req(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, format!("{}{path}", gateway::api()))
            .header("Authorization", &self.token)
            .header("User-Agent", gateway::USER_AGENT)
            .header("X-Super-Properties", gateway::super_properties())
            .header("X-Discord-Locale", "en-US")
    }

    /// Fetches a channel's latest messages, newest API-first; returns them
    /// oldest-first as Mesh wants.
    pub async fn messages(&self, channel_id: &str, state: &tokio::sync::Mutex<State>) -> Result<Value> {
        let resp = self.req(reqwest::Method::GET, &format!("/channels/{channel_id}/messages?limit=50")).send().await.map_err(|e| anyhow!("messages: {e}"))?;
        if !resp.status().is_success() {
            return Err(anyhow!("messages HTTP {}", resp.status()));
        }
        let mut list: Vec<Value> = resp.json().await.map_err(|e| anyhow!("messages json: {e}"))?;
        list.reverse();
        let s = state.lock().await;
        let messages: Vec<Value> = list.iter().map(|m| message(m, &s)).collect();
        Ok(json!({ "channel_id": channel_id, "messages": messages }))
    }

    /// Marks `channel_id` read up to `message_id`, as the client does on viewing it.
    pub async fn ack(&self, channel_id: &str, message_id: &str) -> Result<()> {
        let resp = self.req(reqwest::Method::POST, &format!("/channels/{channel_id}/messages/{message_id}/ack")).json(&json!({ "token": Value::Null })).send().await.map_err(|e| anyhow!("ack: {e}"))?;
        resp.error_for_status().map(drop).map_err(|e| anyhow!("ack HTTP {e}"))
    }

    pub async fn send(&self, channel_id: &str, content: &str) -> Result<()> {
        let resp = self.req(reqwest::Method::POST, &format!("/channels/{channel_id}/messages")).json(&json!({ "content": content })).send().await.map_err(|e| anyhow!("send: {e}"))?;
        resp.error_for_status().map(drop).map_err(|e| anyhow!("send HTTP {e}"))
    }

    /// Downloads a CDN picture (avatars, attachments) through our own network.
    pub async fn fetch_media(&self, url: &str) -> std::result::Result<Vec<u8>, String> {
        let resp = self.http.get(url).header("User-Agent", gateway::USER_AGENT).send().await.map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status()));
        }
        resp.bytes().await.map(|b| b.to_vec()).map_err(|e| e.to_string())
    }

    /// Joins a server from an invite code; returns the guild name if known.
    pub async fn accept_invite(&self, code: &str) -> std::result::Result<Option<String>, String> {
        let code = code.trim().trim_start_matches("https://").trim_start_matches("http://").trim_start_matches("www.");
        let code = code.rsplit('/').next().unwrap_or(code).split(['?', '#']).next().unwrap_or(code);
        let resp = self
            .req(reqwest::Method::POST, &format!("/invites/{code}"))
            .json(&json!({ "session_id": Value::Null }))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let v: Value = resp.json().await.map_err(|e| e.to_string())?;
        if let Some(msg) = v.get("message").and_then(Value::as_str) {
            if v.get("guild").is_none() {
                return Err(msg.to_owned());
            }
        }
        Ok(v.get("guild").and_then(|g| g.get("name")).and_then(Value::as_str).map(str::to_owned))
    }

    /// Our DM with a user, made if there's none yet: the channel object.
    pub async fn open_dm(&self, user_id: &str) -> Result<Value> {
        let resp = self
            .req(reqwest::Method::POST, "/users/@me/channels")
            .json(&json!({ "recipients": [user_id] }))
            .send()
            .await
            .map_err(|e| anyhow!("open DM: {e}"))?;
        resp.error_for_status()
            .map_err(|e| anyhow!("open DM HTTP {e}"))?
            .json()
            .await
            .map_err(|e| anyhow!("open DM: {e}"))
    }

    /// Logs the account out, invalidating this token.
    pub async fn logout(&self) {
        let _ = self.req(reqwest::Method::POST, "/auth/logout").json(&json!({ "provider": Value::Null, "voip_provider": Value::Null })).send().await;
    }
}

// ---------- snapshot ----------

fn avatar(id: &str, hash: Option<&str>) -> Value {
    match hash {
        Some(h) => json!(format!("https://cdn.discordapp.com/avatars/{id}/{h}.png?size=64")),
        None => Value::Null,
    }
}

fn user_obj(id: &str, s: &State) -> Value {
    let u = s.users.get(id);
    let name = u
        .and_then(|u| u.get("global_name").and_then(Value::as_str).filter(|n| !n.is_empty()))
        .or_else(|| u.and_then(|u| u.get("username").and_then(Value::as_str)))
        .unwrap_or(id);
    json!({ "id": id, "name": name, "avatar": avatar(id, u.and_then(|u| u.get("avatar").and_then(Value::as_str))) })
}

/// A snowflake's ms timestamp (Discord epoch 2015-01-01), for DM sorting.
fn snowflake_ms(id: &str) -> u64 {
    id.parse::<u64>().map(|n| (n >> 22) + 1_420_070_400_000).unwrap_or(0)
}

const TEXT_TYPES: [u64; 2] = [0, 5];
/// Discord permission bits.
const ADMINISTRATOR: u64 = 1 << 3;
const VIEW_CHANNEL: u64 = 1 << 10;
const CONNECT: u64 = 1 << 20;

/// Whether we have every bit of `need` in channel `c` of guild `g`: the
/// @everyone role and our roles, then the channel's overwrites for
/// @everyone, our roles, and us (Discord's order). The owner and
/// administrators have everything. With no roles to go on (an old READY),
/// yes: better a channel that fails than one hidden by mistake.
fn may(g: &Value, c: &Value, me: &str, mine: Option<&Vec<String>>, need: u64) -> bool {
    let gid = g.get("id").and_then(Value::as_str).unwrap_or("");
    if g.get("owner_id").and_then(Value::as_str) == Some(me) {
        return true;
    }
    let (Some(roles), Some(mine)) = (g.get("roles").and_then(Value::as_array), mine) else {
        return true;
    };
    let bits = |v: Option<&Value>| {
        v.and_then(Value::as_str)
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0)
    };
    let mut perms = 0;
    for r in roles {
        let id = r.get("id").and_then(Value::as_str).unwrap_or("");
        if id == gid || mine.iter().any(|m| m == id) {
            perms |= bits(r.get("permissions"));
        }
    }
    if perms & ADMINISTRATOR != 0 {
        return true;
    }
    let overwrites: Vec<&Value> = c
        .get("permission_overwrites")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .collect();
    let of = |id: &str| {
        overwrites
            .iter()
            .find(|o| o.get("id").and_then(Value::as_str) == Some(id))
    };
    if let Some(o) = of(gid) {
        perms = (perms & !bits(o.get("deny"))) | bits(o.get("allow"));
    }
    let (mut allow, mut deny) = (0, 0);
    for role in mine {
        if let Some(o) = of(role) {
            (allow, deny) = (allow | bits(o.get("allow")), deny | bits(o.get("deny")));
        }
    }
    perms = (perms & !deny) | allow;
    if let Some(o) = of(me) {
        perms = (perms & !bits(o.get("deny"))) | bits(o.get("allow"));
    }
    perms & need == need
}
const VOICE_TYPES: [u64; 2] = [2, 13];

fn category_name(parent: Option<&str>, s: &State) -> Value {
    parent
        .and_then(|p| s.channels.get(p))
        .and_then(|c| c.get("name").and_then(Value::as_str))
        .map(|n| json!(n))
        .unwrap_or(Value::Null)
}

fn members(channel_id: &str, guild_key: &str, s: &State) -> Vec<Value> {
    let Some(map) = s.voice.get(guild_key) else { return Vec::new() };
    map.values()
        .filter(|vs| vs.get("channel_id").and_then(Value::as_str) == Some(channel_id))
        .filter_map(|vs| {
            let uid = vs.get("user_id").and_then(Value::as_str)?;
            let mut u = user_obj(uid, s);
            if let Some(nick) = vs.get("member").and_then(|m| m.get("nick")).and_then(Value::as_str) {
                u["name"] = json!(nick);
            }
            Some(json!({
                "id": u["id"], "name": u["name"], "avatar": u["avatar"],
                "speaking": s.speaking.get(uid).copied().unwrap_or(false),
                "mute": vs.get("self_mute").and_then(Value::as_bool).unwrap_or(false) || vs.get("mute").and_then(Value::as_bool).unwrap_or(false),
                "deaf": vs.get("self_deaf").and_then(Value::as_bool).unwrap_or(false) || vs.get("deaf").and_then(Value::as_bool).unwrap_or(false),
            }))
        })
        .collect()
}

/// A guild's member list, once it's been asked for (`members`): its groups
/// (online, offline, or a role shown apart) and their members, in order.
fn member_list(gid: &str, g: &Value, s: &State) -> Vec<Value> {
    let rows = s
        .member_lists
        .get(gid)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let role_name = |id: &str| {
        (g.get("roles")
            .and_then(Value::as_array)
            .into_iter()
            .flatten())
        .find(|r| r.get("id").and_then(Value::as_str) == Some(id))
        .and_then(|r| r.get("name").and_then(Value::as_str))
        .map(str::to_owned)
    };
    (rows.iter())
        .filter_map(|row| {
            if let Some(group) = row.get("group") {
                let id = group.get("id").and_then(Value::as_str).unwrap_or("");
                let name = match id {
                    "online" => "Online".to_owned(),
                    "offline" => "Offline".to_owned(),
                    other => role_name(other).unwrap_or_else(|| other.to_owned()),
                };
                return Some(json!({ "group": name, "count": group.get("count") }));
            }
            let m = row.get("member")?;
            let u = m.get("user")?;
            let id = u.get("id").and_then(Value::as_str)?;
            let name = (m.get("nick").and_then(Value::as_str))
                .or_else(|| u.get("global_name").and_then(Value::as_str))
                .or_else(|| u.get("username").and_then(Value::as_str))
                .unwrap_or(id);
            let status = m.get("presence").and_then(|p| p.get("status")).and_then(Value::as_str).unwrap_or("offline");
            Some(json!({ "id": id, "name": name, "avatar": user_obj(id, s)["avatar"], "status": status }))
        })
        .collect()
}

/// The first text channel of a guild we may see: the member list is asked
/// for as seen from it.
pub fn visible_text_channel(s: &State, gid: &str) -> Option<String> {
    let g = s.guilds.get(gid)?;
    let me = s.me_id().unwrap_or_default();
    let mut chans: Vec<&Value> = (s.channels.values())
        .filter(|c| c.get("guild_id").and_then(Value::as_str) == Some(gid))
        .filter(|c| {
            c.get("type")
                .and_then(Value::as_u64)
                .is_some_and(|t| TEXT_TYPES.contains(&t))
        })
        .filter(|c| may(g, c, &me, s.my_roles.get(gid), VIEW_CHANNEL))
        .collect();
    chans.sort_by_key(|c| c.get("position").and_then(Value::as_i64).unwrap_or(0));
    chans
        .first()
        .and_then(|c| c.get("id").and_then(Value::as_str))
        .map(str::to_owned)
}

/// Go Live streams in our voice channel: whose, and whether we watch it.
fn streams(cid: Option<&str>, key: &str, s: &State) -> Vec<Value> {
    let (Some(cid), Some(states)) = (cid, s.voice.get(key)) else {
        return Vec::new();
    };
    let me = s.me_id().unwrap_or_default();
    (states.iter())
        .filter(|(uid, v)| {
            **uid != me
                && v.get("channel_id").and_then(Value::as_str) == Some(cid)
                && v.get("self_stream").and_then(Value::as_bool) == Some(true)
        })
        .map(|(uid, _)| {
            let guild = (key != "@me").then_some(key);
            let watching = s.watching.contains(&gateway::stream_key(guild, cid, uid));
            json!({ "owner_id": uid, "name": user_obj(uid, s)["name"], "watching": watching })
        })
        .collect()
}

/// Builds the full Mesh snapshot from the current gateway state.
pub fn snapshot(s: &State) -> Value {
    let me = s.me.as_ref().map(|m| {
        let id = m.get("id").and_then(Value::as_str).unwrap_or("");
        user_obj(id, s)
    });

    let mut guilds: Vec<Value> = s
        .guilds
        .values()
        .map(|g| {
            let gid = g.get("id").and_then(Value::as_str).unwrap_or("");
            let me = s.me_id().unwrap_or_default();
            let mine = s.my_roles.get(gid);
            let locked = |c: &Value, need: u64| !may(g, c, &me, mine, need);
            let mut chans: Vec<&Value> = s.channels.values().filter(|c| c.get("guild_id").and_then(Value::as_str) == Some(gid)).collect();
            chans.sort_by_key(|c| c.get("position").and_then(Value::as_i64).unwrap_or(0));
            let text: Vec<Value> = chans
                .iter()
                .filter(|c| c.get("type").and_then(Value::as_u64).is_some_and(|t| TEXT_TYPES.contains(&t)))
                .map(|c| json!({ "id": c["id"], "name": c["name"], "category": category_name(c.get("parent_id").and_then(Value::as_str), s), "locked": locked(c, VIEW_CHANNEL) }))
                .collect();
            let voice: Vec<Value> = chans
                .iter()
                .filter(|c| c.get("type").and_then(Value::as_u64).is_some_and(|t| VOICE_TYPES.contains(&t)))
                .map(|c| {
                    let cid = c.get("id").and_then(Value::as_str).unwrap_or("");
                    json!({ "id": c["id"], "name": c["name"], "category": category_name(c.get("parent_id").and_then(Value::as_str), s), "members": members(cid, gid, s), "locked": locked(c, VIEW_CHANNEL | CONNECT) })
                })
                .collect();
            json!({
                "id": gid, "name": g.get("name").cloned().unwrap_or(Value::Null),
                "icon": g.get("icon").and_then(Value::as_str).map(|h| json!(format!("https://cdn.discordapp.com/icons/{gid}/{h}.png?size=64"))).unwrap_or(Value::Null),
                "text_channels": text, "voice_channels": voice,
                "member_list": member_list(gid, g, s),
            })
        })
        .collect();
    guilds.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));

    let mut dms: Vec<Value> = s
        .dm_channels
        .iter()
        .map(|ch| {
            let id = ch.get("id").and_then(Value::as_str).unwrap_or("");
            // User objects, or (a user account's READY) just their ids, with the
            // users themselves in READY's `users`.
            let ids = ch.get("recipients").and_then(Value::as_array).into_iter().flatten().filter_map(|r| r.get("id").and_then(Value::as_str));
            let bare = ch.get("recipient_ids").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str);
            let recipients: Vec<Value> = ids.chain(bare).map(|rid| user_obj(rid, s)).collect();
            let name = ch
                .get("name")
                .and_then(Value::as_str)
                .filter(|n| !n.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| recipients.iter().filter_map(|r| r["name"].as_str()).collect::<Vec<_>>().join(", "));
            let last = s.channels.get(id).and_then(|c| c.get("last_message_id")).and_then(Value::as_str).or_else(|| ch.get("last_message_id").and_then(Value::as_str));
            let (read_last, mentions) = s.read_state.get(id).cloned().unwrap_or_default();
            let unread = last.is_some_and(|l| snowflake_ms(l) > snowflake_ms(&read_last));
            json!({
                "id": id,
                "name": if name.is_empty() { "Direct message".into() } else { name },
                "recipient_id": if recipients.len() == 1 { recipients[0]["id"].clone() } else { Value::Null },
                "avatar": if recipients.len() == 1 { recipients[0]["avatar"].clone() } else { Value::Null },
                "last_ms": last.map(snowflake_ms).unwrap_or(0),
                "unread": unread,
                "mentions": mentions,
            })
        })
        .collect();
    dms.sort_by(|a, b| b["last_ms"].as_u64().cmp(&a["last_ms"].as_u64()));

    let voice = {
        let cid = s.voice_channel.clone();
        let name = cid
            .as_ref()
            .and_then(|c| s.channels.get(c))
            .and_then(|c| c.get("name").and_then(Value::as_str))
            .map(str::to_owned);
        let key = s.voice_guild.clone().unwrap_or_else(|| "@me".into());
        json!({
            "channel_id": cid, "guild_id": s.voice_guild, "channel_name": name.or_else(|| cid.as_ref().map(|_| "call".into())),
            "connection": if s.rtc_connected { "RTC_CONNECTED" } else { "" },
            "self_mute": s.self_mute, "self_deaf": s.self_deaf, "self_video": s.self_video,
            "streams": streams(cid.as_deref(), &key, s), "self_stream": s.sharing,
            "members": cid.as_deref().map(|c| members(c, &key, s)).unwrap_or_default(),
        })
    };

    // Calls ringing us that we haven't joined, named like their DM.
    let me_id = s.me_id();
    let ringing: Vec<Value> = s
        .calls
        .iter()
        .filter(|(cid, ids)| me_id.as_ref().is_some_and(|me| ids.contains(me)) && s.voice_channel.as_ref() != Some(*cid))
        .map(|(cid, _)| {
            let dm = dms.iter().find(|d| d["id"] == cid.as_str());
            json!({ "channel_id": cid, "name": dm.map(|d| d["name"].clone()).unwrap_or(json!("Discord call")) })
        })
        .collect();

    json!({ "me": me, "voice": voice, "guilds": guilds, "dms": dms, "ringing": ringing })
}

// Discord message types Mesh draws as plain text; the rest are system lines.
const TEXT_MSG: [u64; 4] = [0, 19, 20, 21];

/// `<@id>` / `<@!id>` as `@name`, for the users the message says it mentions.
fn named_mentions(content: &str, mentions: Option<&Value>) -> String {
    let mut out = content.to_owned();
    for u in mentions.and_then(Value::as_array).into_iter().flatten() {
        let Some(id) = u.get("id").and_then(Value::as_str) else { continue };
        let name = u.get("global_name").and_then(Value::as_str).filter(|n| !n.is_empty()).or_else(|| u.get("username").and_then(Value::as_str)).unwrap_or(id);
        out = out.replace(&format!("<@{id}>"), &format!("@{name}")).replace(&format!("<@!{id}>"), &format!("@{name}"));
    }
    out
}

fn message(m: &Value, s: &State) -> Value {
    let author = m.get("author");
    let aid = author.and_then(|a| a.get("id")).and_then(Value::as_str).unwrap_or("");
    let mtype = m.get("type").and_then(Value::as_u64).unwrap_or(0);
    let is_text = TEXT_MSG.contains(&mtype);
    let name = author
        .and_then(|a| a.get("global_name").and_then(Value::as_str).filter(|n| !n.is_empty()))
        .or_else(|| author.and_then(|a| a.get("username").and_then(Value::as_str)))
        .unwrap_or(aid);
    let content = if is_text {
        named_mentions(m.get("content").and_then(Value::as_str).unwrap_or(""), m.get("mentions"))
    } else {
        system_line(mtype)
    };
    let stickers = m.get("sticker_items").and_then(Value::as_array).into_iter().flatten().filter_map(sticker);
    let attachments: Vec<Value> = m.get("attachments").and_then(Value::as_array).into_iter().flatten().map(attachment).chain(stickers).collect();
    let _ = s;
    json!({
        "id": m.get("id").cloned().unwrap_or(Value::Null),
        "author_id": aid,
        "author": name,
        "avatar": avatar(aid, author.and_then(|a| a.get("avatar").and_then(Value::as_str))),
        "content": content,
        "system": !is_text,
        "timestamp_ms": m.get("id").and_then(Value::as_str).map(snowflake_ms).unwrap_or(0),
        "attachments": attachments,
    })
}

fn system_line(t: u64) -> String {
    match t {
        1 => "added someone",
        2 => "removed someone",
        3 => "started a call",
        6 => "pinned a message",
        7 => "joined the server",
        _ => "did something Mesh can't show yet",
    }
    .to_owned()
}

/// A sticker as a 160 px picture: PNG and APNG (animated) from the CDN as they are, GIF
/// through the media proxy; Lottie ones (format 3) aren't pictures, so they're left out.
fn sticker(s: &Value) -> Option<Value> {
    let id = s.get("id").and_then(Value::as_str)?;
    let url = match s.get("format_type").and_then(Value::as_u64) {
        Some(3) => return None,
        Some(4) => format!("https://media.discordapp.net/stickers/{id}.gif?size=160"),
        _ => format!("https://cdn.discordapp.com/stickers/{id}.png?size=160"),
    };
    Some(json!({ "filename": s.get("name").cloned().unwrap_or(json!("sticker")), "url": url, "preview": url, "width": 160, "height": 160, "size": Value::Null }))
}

fn attachment(a: &Value) -> Value {
    let width = a.get("width").and_then(Value::as_u64);
    let height = a.get("height").and_then(Value::as_u64);
    let is_image = a.get("content_type").and_then(Value::as_str).is_some_and(|c| c.starts_with("image/")) && width.is_some() && height.is_some();
    let preview = if is_image {
        let (w, h) = (width.unwrap(), height.unwrap());
        let scale = (480.0 / w as f64).min(1.0);
        a.get("proxy_url").and_then(Value::as_str).map(|p| {
            let sep = if p.contains('?') { '&' } else { '?' };
            json!(format!("{p}{sep}width={}&height={}", (w as f64 * scale) as u64, (h as f64 * scale) as u64))
        })
    } else {
        None
    };
    json!({
        "filename": a.get("filename").cloned().unwrap_or(Value::Null),
        "url": a.get("url").cloned().unwrap_or(Value::Null),
        "preview": preview.unwrap_or(Value::Null),
        "width": width, "height": height, "size": a.get("size").and_then(Value::as_u64),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Discord's order: @everyone and roles, then @everyone's, the roles'
    /// and our own overwrites; the owner and admins see everything.
    /// Go Live: someone else's `self_stream` in our channel is a stream we
    /// can watch (or are watching); ours and other channels' aren't listed.
    #[test]
    fn streams_in_our_channel() {
        let mut s = State::default();
        s.me = Some(json!({ "id": "me" }));
        let vs = |c: &str, live: bool| json!({ "channel_id": c, "self_stream": live });
        let mut g = std::collections::HashMap::new();
        g.insert("me".to_string(), vs("c", true));
        g.insert("ann".to_string(), vs("c", true));
        g.insert("ben".to_string(), vs("c", false));
        g.insert("cal".to_string(), vs("other", true));
        s.voice.insert("g".to_string(), g);
        s.watching
            .insert(gateway::stream_key(Some("g"), "c", "ann"));
        let listed = streams(Some("c"), "g", &s);
        assert_eq!(listed.len(), 1, "{listed:?}");
        assert_eq!(
            (
                listed[0]["owner_id"].as_str(),
                listed[0]["watching"].as_bool()
            ),
            (Some("ann"), Some(true))
        );
        assert_eq!(gateway::stream_key(None, "c", "ann"), "call:c:ann");
    }

    #[test]
    fn who_may_see_and_join() {
        let g = json!({ "id": "g", "owner_id": "boss", "roles": [
            { "id": "g", "permissions": (VIEW_CHANNEL | CONNECT).to_string() },
            { "id": "mod", "permissions": "0" },
            { "id": "admin", "permissions": ADMINISTRATOR.to_string() },
        ]});
        let open = json!({ "id": "c1" });
        let hidden = json!({ "id": "c2", "permission_overwrites": [
            { "id": "g", "type": 0, "allow": "0", "deny": VIEW_CHANNEL.to_string() },
            { "id": "mod", "type": 0, "allow": VIEW_CHANNEL.to_string(), "deny": "0" },
        ]});
        let mine_only = json!({ "id": "c3", "permission_overwrites": [
            { "id": "g", "type": 0, "allow": "0", "deny": VIEW_CHANNEL.to_string() },
            { "id": "me", "type": 1, "allow": VIEW_CHANNEL.to_string(), "deny": "0" },
        ]});
        let no_voice = json!({ "id": "c4", "permission_overwrites": [
            { "id": "g", "type": 0, "allow": "0", "deny": CONNECT.to_string() },
        ]});
        let plain: Vec<String> = vec![];
        let modr = vec!["mod".to_string()];
        let admin = vec!["admin".to_string()];
        assert!(may(&g, &open, "me", Some(&plain), VIEW_CHANNEL));
        assert!(
            !may(&g, &hidden, "me", Some(&plain), VIEW_CHANNEL),
            "@everyone denied"
        );
        assert!(
            may(&g, &hidden, "me", Some(&modr), VIEW_CHANNEL),
            "a role's allow wins over @everyone's deny"
        );
        assert!(
            may(&g, &mine_only, "me", Some(&plain), VIEW_CHANNEL),
            "our own overwrite last"
        );
        assert!(may(&g, &no_voice, "me", Some(&plain), VIEW_CHANNEL));
        assert!(
            !may(&g, &no_voice, "me", Some(&plain), VIEW_CHANNEL | CONNECT),
            "seen but can't join"
        );
        assert!(
            may(&g, &hidden, "me", Some(&admin), VIEW_CHANNEL | CONNECT),
            "admin"
        );
        assert!(
            may(&g, &hidden, "boss", Some(&plain), VIEW_CHANNEL),
            "owner"
        );
        assert!(
            may(&g, &hidden, "me", None, VIEW_CHANNEL),
            "no roles known: not locked"
        );
    }

    #[test]
    fn mentions_show_names() {
        let ms = json!([{ "id": "7", "username": "ann", "global_name": "Ann" }, { "id": "8", "username": "bob" }]);
        assert_eq!(
            named_mentions("hi <@7> and <@!8>, not <@9>", Some(&ms)),
            "hi @Ann and @bob, not <@9>"
        );
    }

    #[test]
    fn snapshot_shape_matches_mesh() {
        let mut s = State::default();
        s.me = Some(json!({ "id": "1", "username": "me", "global_name": "Me", "avatar": null }));
        s.users.insert("1".into(), s.me.clone().unwrap());
        s.guilds.insert(
            "10".into(),
            json!({ "id": "10", "name": "Guild", "icon": null }),
        );
        s.channels.insert(
            "100".into(),
            json!({ "id": "100", "name": "general", "type": 0, "guild_id": "10", "position": 1 }),
        );
        s.channels.insert(
            "101".into(),
            json!({ "id": "101", "name": "Voice", "type": 2, "guild_id": "10", "position": 2 }),
        );
        s.dm_channels.push(json!({ "id": "500", "type": 1, "recipients": [{ "id": "2" }], "last_message_id": "900" }));
        s.dm_channels.push(
            json!({ "id": "501", "type": 1, "recipient_ids": ["2"], "last_message_id": "901" }),
        );
        s.users.insert(
            "2".into(),
            json!({ "id": "2", "username": "friend", "avatar": null }),
        );

        let snap = snapshot(&s);
        // Parse through Mesh's own type to prove the fields line up.
        let parsed: mesh_snapshot::Snapshot = serde_json::from_value(snap).unwrap();
        assert_eq!(parsed.me.unwrap().name, "Me");
        assert_eq!(parsed.guilds[0].text_channels[0].name, "general");
        assert_eq!(parsed.guilds[0].voice_channels[0].name, "Voice");
        assert_eq!(parsed.dms[0].recipient_id.as_deref(), Some("2"));
        assert!(
            parsed
                .dms
                .iter()
                .all(|d| d.recipient_id.as_deref() == Some("2")),
            "recipient_ids DMs resolve too"
        );

        // A call ringing us shows, named like its DM; one ringing someone else doesn't.
        s.calls.insert("500".into(), vec!["1".into()]);
        s.calls.insert("501".into(), vec!["2".into()]);
        let parsed: mesh_snapshot::Snapshot = serde_json::from_value(snapshot(&s)).unwrap();
        assert_eq!(parsed.ringing.len(), 1);
        assert_eq!(
            (
                parsed.ringing[0].channel_id.as_str(),
                parsed.ringing[0].name.as_str()
            ),
            ("500", "friend")
        );
    }

    #[test]
    fn messages_are_oldest_first_and_system_lines_named() {
        let s = State::default();
        let raw = json!({ "id": "900", "type": 3, "author": { "id": "2", "username": "x", "avatar": null }, "content": "", "attachments": [] });
        let with_stickers = json!({ "id": "901", "type": 0, "author": { "id": "2", "username": "x", "avatar": null }, "content": "",
            "sticker_items": [{ "id": "7", "name": "wave", "format_type": 2 }, { "id": "8", "format_type": 3 }] });
        let atts = message(&with_stickers, &State::default())["attachments"].clone();
        assert_eq!(
            atts.as_array().unwrap().len(),
            1,
            "lottie stickers are left out"
        );
        assert_eq!(
            atts[0]["preview"],
            "https://cdn.discordapp.com/stickers/7.png?size=160"
        );
        let m = message(&raw, &s);
        assert_eq!(m["system"], true);
        assert_eq!(m["content"], "started a call");
    }

    // Minimal mirror of the Mesh structs the snapshot must deserialize into.
    mod mesh_snapshot {
        use serde::Deserialize;
        #[derive(Deserialize)]
        pub struct Snapshot {
            pub me: Option<User>,
            pub guilds: Vec<Guild>,
            pub dms: Vec<Dm>,
            pub ringing: Vec<Ring>,
        }
        #[derive(Deserialize)]
        pub struct Ring {
            pub channel_id: String,
            pub name: String,
        }
        #[derive(Deserialize)]
        pub struct User {
            pub name: String,
        }
        #[derive(Deserialize)]
        pub struct Guild {
            pub text_channels: Vec<Chan>,
            pub voice_channels: Vec<Chan>,
        }
        #[derive(Deserialize)]
        pub struct Chan {
            pub name: String,
        }
        #[derive(Deserialize)]
        pub struct Dm {
            pub recipient_id: Option<String>,
        }
    }
}
