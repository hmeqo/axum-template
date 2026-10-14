//! 长连接帧契约：保留的控制帧名、上行解析、下行信封。
//!
//! 保留名见 [`RESERVED_TYPES`]：业务帧的 `type` 不得与它们重名，否则业务帧被当成控制帧处理。
//! `resync` 表示服务端发现订阅者落后（旧事件已被挤掉），客户端收到即回源补拉，旧事件不补发。

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    app::http::error::{ErrorResp, body},
    error::{AppError, ErrorCode},
    infra::realtime::Topic,
};

/// 保留的控制帧名。
pub const RESERVED_TYPES: &[&str] = &[
    "subscribe",
    "unsubscribe",
    "ping",
    "pong",
    "resync",
    "error",
];

/// 客户端 → 服务端：控制帧。
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlFrame {
    Subscribe { topic: Topic },
    Unsubscribe { topic: Topic },
    Ping,
}

/// 上行帧：控制帧，或交给业务的帧。
#[derive(Debug)]
pub enum Inbound<E> {
    Control(ControlFrame),
    Business(E),
}

/// 先按保留名分流，再核对控制帧形状，最后才交给业务类型；
/// 都不认时给出 `realtime.unknown_frame`（消息里带上原始的 `type`）。
pub fn parse_inbound<E: DeserializeOwned>(raw: &str) -> Result<Inbound<E>, AppError> {
    let kind = frame_kind(raw);

    if RESERVED_TYPES.contains(&kind) {
        return match control_frame(raw, kind) {
            Some(frame) => Ok(Inbound::Control(frame)),
            None => {
                tracing::debug!(kind, "malformed control frame");
                Err(unknown_frame(kind))
            }
        };
    }

    match serde_json::from_str::<E>(raw) {
        Ok(event) => Ok(Inbound::Business(event)),
        Err(error) => {
            tracing::debug!(kind, %error, "unrecognized realtime frame");
            Err(unknown_frame(kind))
        }
    }
}

/// 保留名下的控制帧：字段集合必须与该变体完全一致——serde 对内部 tag 的枚举不执行
/// `deny_unknown_fields`。
fn control_frame(raw: &str, kind: &str) -> Option<ControlFrame> {
    let serde_json::Value::Object(fields) = serde_json::from_str(raw).ok()? else {
        return None;
    };

    let expected: &[&str] = match kind {
        "subscribe" | "unsubscribe" => &["topic", "type"],
        _ => &["type"],
    };
    if fields.len() != expected.len() || !fields.keys().all(|key| expected.contains(&key.as_str()))
    {
        return None;
    }

    serde_json::from_value(serde_json::Value::Object(fields)).ok()
}

/// 服务端 → 客户端：业务事件带 `topic`，控制帧只有固定几种。
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ServerFrame<E> {
    /// 业务负载必须序列化成自描述的 map（`type` 由业务类型自己带）。
    Event {
        topic: Topic,
        #[serde(flatten)]
        event: E,
    },
    Pong(PongFrame),
    Resync(ResyncFrame),
    Error(ErrorFrame),
}

impl<E> ServerFrame<E> {
    /// 应用级 ping 的应答：浏览器不能发协议层 ping 帧。
    pub fn pong() -> Self {
        Self::Pong(PongFrame { kind: "pong" })
    }

    /// 订阅落后：客户端应回源补拉，旧事件不补发。
    pub fn resync(topic: Topic) -> Self {
        Self::Resync(ResyncFrame {
            kind: "resync",
            topic,
        })
    }

    /// 与 REST 同一份 `{code?, message}` 映射。
    pub fn error(error: &AppError) -> Self {
        Self::Error(error.into())
    }
}

#[derive(Debug, Serialize)]
pub struct PongFrame {
    #[serde(rename = "type")]
    kind: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ResyncFrame {
    #[serde(rename = "type")]
    kind: &'static str,
    topic: Topic,
}

#[derive(Debug, Serialize)]
pub struct ErrorFrame {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(flatten)]
    resp: ErrorResp,
}

impl From<&AppError> for ErrorFrame {
    fn from(error: &AppError) -> Self {
        Self {
            kind: "error",
            resp: body(error),
        }
    }
}

/// 只读 `type` 字段，借用输入、不建树；`type` 缺失或不是字符串时得空串。
fn frame_kind(raw: &str) -> &str {
    #[derive(Deserialize)]
    struct Header<'a> {
        #[serde(rename = "type", borrow)]
        kind: &'a str,
    }

    serde_json::from_str::<Header<'_>>(raw).map_or("", |header| header.kind)
}

fn unknown_frame(kind: &str) -> AppError {
    let message = if kind.is_empty() {
        "Unrecognized realtime frame: missing `type`".to_owned()
    } else {
        format!("Unrecognized realtime frame: `{kind}`")
    };

    AppError::business_msg(ErrorCode::RealtimeUnknownFrame, message)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum Command {
        Chat { text: String },
    }

    #[derive(Debug, Serialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum Event {
        Chat { from: i64, text: String },
    }

    fn inbound(raw: &str) -> Result<Inbound<Command>, AppError> {
        parse_inbound(raw)
    }

    #[test]
    fn every_control_frame_wears_a_reserved_name() {
        assert_eq!(
            RESERVED_TYPES,
            [
                "subscribe",
                "unsubscribe",
                "ping",
                "pong",
                "resync",
                "error"
            ]
        );

        // 三个保留名是客户端帧，另三个只由服务端发出。
        let client_side = ["subscribe", "unsubscribe", "ping"];
        for name in RESERVED_TYPES {
            let raw = if *name == "subscribe" || *name == "unsubscribe" {
                format!(r#"{{"type":"{name}","topic":"lobby"}}"#)
            } else {
                format!(r#"{{"type":"{name}"}}"#)
            };

            assert_eq!(
                serde_json::from_str::<ControlFrame>(&raw).is_ok(),
                client_side.contains(name),
                "{raw}"
            );
        }
    }

    #[test]
    fn inbound_frames_split_between_control_and_business() {
        assert!(matches!(
            inbound(r#"{"type":"subscribe","topic":"lobby"}"#).unwrap(),
            Inbound::Control(ControlFrame::Subscribe { topic }) if topic.as_str() == "lobby"
        ));
        assert!(matches!(
            inbound(r#"{"type":"unsubscribe","topic":"lobby"}"#).unwrap(),
            Inbound::Control(ControlFrame::Unsubscribe { topic }) if topic.as_str() == "lobby"
        ));
        assert!(matches!(
            inbound(r#"{"type":"ping"}"#).unwrap(),
            Inbound::Control(ControlFrame::Ping)
        ));
        assert!(matches!(
            inbound(r#"{"type":"chat","text":"hi"}"#).unwrap(),
            Inbound::Business(Command::Chat { text }) if text == "hi"
        ));
    }

    #[test]
    fn an_unrecognized_frame_reports_its_type() {
        for raw in [r#"{"type":"nope"}"#, "{", r#"{"text":"no type"}"#] {
            match inbound(raw).unwrap_err() {
                AppError::Business { code, message, .. } => {
                    assert_eq!(code, ErrorCode::RealtimeUnknownFrame, "{raw}");
                    assert!(!message.is_empty(), "{raw}");
                }
                other => panic!("{raw} must be a client-visible failure, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_reserved_type_is_never_a_business_frame() {
        assert!(inbound(r#"{"type":"ping","x":1}"#).is_err());
        assert!(inbound(r#"{"type":"subscribe"}"#).is_err());
        assert!(inbound(r#"{"type":"chat","text":"hi","x":1}"#).is_ok());
    }

    #[test]
    fn server_frames_serialize_as_documented() {
        let event = ServerFrame::Event {
            topic: Topic::new("lobby"),
            event: Event::Chat {
                from: 7,
                text: "hi".into(),
            },
        };
        assert_eq!(
            serde_json::to_value(&event).unwrap(),
            json!({"type": "chat", "topic": "lobby", "from": 7, "text": "hi"})
        );
        assert_eq!(
            serde_json::to_value(ServerFrame::<Event>::pong()).unwrap(),
            json!({"type": "pong"})
        );
        assert_eq!(
            serde_json::to_value(ServerFrame::<Event>::resync(Topic::new("lobby"))).unwrap(),
            json!({"type": "resync", "topic": "lobby"})
        );
    }

    #[test]
    fn an_error_frame_carries_the_rest_body() {
        let error = AppError::business_msg(ErrorCode::RealtimeTopicForbidden, "no such topic");
        let frame = ServerFrame::<Event>::error(&error);

        assert_eq!(
            serde_json::to_value(&frame).unwrap(),
            json!({
                "type": "error",
                "code": "realtime.topic_forbidden",
                "message": "no such topic",
            })
        );
        assert_eq!(body(&error).code, Some(ErrorCode::RealtimeTopicForbidden));
    }
}
