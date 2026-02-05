use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrintLevel {
    Pickup,
    Obituary,
    High,
    Chat,
    TeamChat,
    ServerChat,
    Warning,
    Error,
    // These exist in Odamex's code, but are only for special handling in the game client
    // NoRCON,
    // FilterChat,
    // FilterHigh
    // MaxPrint
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "content", rename_all = "snake_case")]
pub enum ServerMessageType {
    LoginResponse(u64),
    LoginFailure(String),
    LoginSuccess,
    Print {
        printlevel: PrintLevel,
        text: String,
    },
    Maplist,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProtocolVersion {
    pub major: u8,
    pub minor: u8,
    pub revision: u8,
}

impl Serialize for ProtocolVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer
            .serialize_str(format!("{}.{}.{}", self.major, self.minor, self.revision).as_ref())
    }
}

impl<'a> Deserialize<'a> for ProtocolVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        let s = String::deserialize(deserializer)?;
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(serde::de::Error::custom(
                "Expected format 'major.minor.revision'",
            ));
        }

        let major = parts[0]
            .parse::<u8>()
            .map_err(|_| serde::de::Error::custom("Invalid major version"))?;
        let minor = parts[1]
            .parse::<u8>()
            .map_err(|_| serde::de::Error::custom("Invalid minor version"))?;
        let revision = parts[2]
            .parse::<u8>()
            .map_err(|_| serde::de::Error::custom("Invalid revision version"))?;

        Ok(ProtocolVersion {
            major,
            minor,
            revision,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "content", rename_all = "snake_case")]
pub enum ClientMessageType {
    LoginRequest(ProtocolVersion),
    LoginPassword(String),
    Command(String),
    Maplist,
}

pub trait MessageContent {}
impl MessageContent for ServerMessageType {}
impl MessageContent for ClientMessageType {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Message<T: MessageContent> {
    #[serde(flatten)]
    pub content: T,
    pub id: usize,
}

pub type ServerMessage = Message<ServerMessageType>;
pub type ClientMessage = Message<ClientMessageType>;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deserialize_print() {
        let json = json!({
            "type": "print",
            "id": 2,
            "content": {
                "printlevel": "high",
                "text": "Hello, world!"
            }
        });
        let print = serde_json::from_value::<ServerMessage>(json).unwrap();
        assert_eq!(
            print,
            ServerMessage {
                content: ServerMessageType::Print {
                    printlevel: PrintLevel::High,
                    text: "Hello, world!".to_string()
                },
                id: 2,
            }
        );
    }

    #[test]
    fn deserialize_login_response() {
        let json = json!({
            "type": "login_response",
            "id": 2,
            "content": 2345234
        });
        let print = serde_json::from_value::<ServerMessage>(json).unwrap();
        assert_eq!(
            print,
            ServerMessage {
                content: ServerMessageType::LoginResponse(2345234),
                id: 2,
            }
        );
    }

    #[test]
    fn deserialize_login_success() {
        let json = json!({
            "type": "login_success",
            "id": 2,
            "content": null
        });
        let print = serde_json::from_value::<ServerMessage>(json).unwrap();
        assert_eq!(
            print,
            ServerMessage {
                content: ServerMessageType::LoginSuccess,
                id: 2,
            }
        );
    }

    #[test]
    fn deserialize_login_failure() {
        let json = json!({
            "type": "login_failure",
            "id": 2,
            "content": "wrong password dude"
        });
        let print = serde_json::from_value::<ServerMessage>(json).unwrap();
        assert_eq!(
            print,
            ServerMessage {
                content: ServerMessageType::LoginFailure("wrong password dude".to_string()),
                id: 2,
            }
        );
    }

    #[test]
    fn serialize_command() {
        let command = ClientMessage {
            content: ClientMessageType::Command("echo hello".to_string()),
            id: 1,
        };
        let json = serde_json::to_value(&command).unwrap();
        assert_eq!(
            json,
            json!({
                "type": "command",
                "id": 1,
                "content": "echo hello"
            })
        );
    }

    #[test]
    fn serialize_login_request() {
        let command = ClientMessage {
            content: ClientMessageType::LoginRequest(ProtocolVersion {
                major: 1,
                minor: 0,
                revision: 0,
            }),
            id: 5,
        };
        let json = serde_json::to_value(&command).unwrap();
        assert_eq!(
            json,
            json!({
                "type": "login_request",
                "id": 5,
                "content": "1.0.0"
            })
        );
    }

    #[test]
    fn serialize_login_password() {
        let command = ClientMessage {
            content: ClientMessageType::LoginPassword("password".to_string()),
            id: 20,
        };
        let json = serde_json::to_value(&command).unwrap();
        assert_eq!(
            json,
            json!({
                "type": "login_password",
                "id": 20,
                "content": "password"
            })
        );
    }
}
