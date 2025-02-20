use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use tokio::{
    net::TcpStream,
    sync::{Mutex, MutexGuard, mpsc},
    time,
};
use tokio_tungstenite::{
    MaybeTlsStream,
    tungstenite::{self, error::ProtocolError, http::Uri, protocol::WebSocketConfig},
};
use tracing::{error, info, trace, warn};
use twitch_api::{
    HelixClient,
    eventsub::{
        Event, EventType, EventsubWebsocketData, Message, Payload, ReconnectPayload, SessionData,
        Transport, WelcomePayload,
        channel::{ChannelChatMessageV1, ChannelChatMessageV1Payload},
        stream::{StreamOfflineV1, StreamOnlineV1},
    },
    helix::chat::{SendChatMessageBody, SendChatMessageRequest},
    twitch_oauth2::{TwitchToken, UserToken, client::Client as Oauth2Client},
    types::{MsgId, UserId},
};

use crate::twitch::StreamInfo;

type WebSocketStream = tokio_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>;

pub struct EventSubClient {
    session_id: Option<String>,
    streamer_id: UserId,
    user_id: UserId,
    client: HelixClient<'static, reqwest::Client>,
    token: Token,
    connect_url: Uri,
    connection: WebSocketStream,
}

impl EventSubClient {
    pub async fn new(
        client: HelixClient<'static, reqwest::Client>,
        token: UserToken,
        streamer_id: UserId,
    ) -> Result<Self> {
        let url = Uri::from_static(twitch_api::TWITCH_EVENTSUB_WEBSOCKET_URL.as_str());
        let connection = Self::connect(&url).await?;

        Ok(Self {
            session_id: None,
            streamer_id,
            user_id: token.user_id.clone(),
            client,
            token: Token::new(token),
            connect_url: url,
            connection,
        })
    }

    pub fn create_replier(&self) -> Replier {
        Replier {
            streamer_id: self.streamer_id.clone(),
            user_id: self.user_id.clone(),
            client: self.client.clone(),
            token: self.token.clone(),
        }
    }

    async fn connect(url: &Uri) -> Result<WebSocketStream> {
        let (stream, _) = tokio_tungstenite::connect_async_with_config(
            url,
            Some(
                WebSocketConfig::default()
                    .max_message_size(Some(64 << 20)) // 64 MiB
                    .max_frame_size(Some(16 << 20)) // 16 MiB
                    .accept_unmasked_frames(false),
            ),
            false,
        )
        .await?;

        Ok(stream)
    }

    async fn reconnect(url: &Uri) -> Result<WebSocketStream> {
        let mut delay = Duration::ZERO;

        while delay <= Duration::from_secs(10) {
            match Self::connect(url).await {
                Ok(stream) => return Ok(stream),
                Err(err) => warn!(?err, ?delay, "failed reconnecting"),
            }

            delay += Duration::from_secs(1);
            time::sleep(delay).await;
        }

        bail!("gave up reconnecting")
    }

    pub async fn start(&mut self, tx: mpsc::Sender<ChannelChatMessageV1Payload>) -> Result<()> {
        while let Some(message) = self.connection.next().await {
            let message = match message {
                Err(tungstenite::Error::Protocol(ProtocolError::ResetWithoutClosingHandshake)) => {
                    self.connection = Self::reconnect(&self.connect_url).await?;
                    continue;
                }
                Err(e) => {
                    warn!(error = ?e, "failed receiving message");
                    self.connection = Self::reconnect(&Uri::from_static(
                        twitch_api::TWITCH_EVENTSUB_WEBSOCKET_URL.as_str(),
                    ))
                    .await?;
                    continue;
                }
                Ok(message) => message,
            };

            if let Err(err) = self.process_websocket_message(message, tx.clone()).await {
                error!(?err, "failed processing message");
            }
        }

        Ok(())
    }

    pub async fn process_websocket_message(
        &mut self,
        msg: tungstenite::Message,
        tx: mpsc::Sender<ChannelChatMessageV1Payload>,
    ) -> Result<()> {
        match msg {
            tungstenite::Message::Text(text) => {
                self.process_eventsub_message(Event::parse_websocket(&text)?, tx)
                    .await
            }
            tungstenite::Message::Ping(msg) => self
                .connection
                .send(tungstenite::Message::Pong(msg))
                .await
                .map_err(Into::into),
            tungstenite::Message::Close(_) => {
                self.connection = Self::reconnect(&Uri::from_static(
                    twitch_api::TWITCH_EVENTSUB_WEBSOCKET_URL.as_str(),
                ))
                .await?;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    async fn process_eventsub_message(
        &mut self,
        data: EventsubWebsocketData<'_>,
        tx: mpsc::Sender<ChannelChatMessageV1Payload>,
    ) -> Result<()> {
        match data {
            EventsubWebsocketData::Welcome {
                payload: WelcomePayload { session },
                ..
            }
            | EventsubWebsocketData::Reconnect {
                payload: ReconnectPayload { session },
                ..
            } => self.process_welcome_message(session).await,
            EventsubWebsocketData::Notification { payload, .. } => {
                self.process_notification_message(payload, tx).await
            }
            EventsubWebsocketData::Revocation { metadata, payload } => {
                warn!(?metadata, ?payload, "received revocation");
                Ok(())
            }
            _ => Ok(()),
        }
    }

    #[allow(clippy::unused_async, clippy::unnecessary_wraps)]
    async fn process_notification_message(
        &self,
        event: Event,
        tx: mpsc::Sender<ChannelChatMessageV1Payload>,
    ) -> Result<()> {
        match event {
            Event::StreamOnlineV1(Payload {
                message: Message::Notification(message),
                ..
            }) => {
                let get_info = async || {
                    let token = self.token.get(&self.client).await.ok()?;
                    let stream = self
                        .client
                        .get_streams_from_ids(&[&message.broadcaster_user_id][..].into(), &*token)
                        .next()
                        .await?
                        .ok()?;

                    StreamInfo::try_from(stream).ok()
                };

                if let Some(info) = get_info().await {
                    info!(
                        info.id,
                        %info.started_at,
                        info.title,
                        info.category,
                        "streamer started streaming",
                    );
                } else {
                    info!(
                        info.id = message.id,
                        info.started_at = %message.started_at,
                        "streamer started streaming",
                    );
                }
            }
            Event::StreamOfflineV1(Payload {
                message: Message::Notification(_),
                ..
            }) => {
                info!("streamer stopped streaming");
            }
            Event::ChannelChatMessageV1(Payload {
                message: Message::Notification(message),
                ..
            }) => {
                if message.chatter_user_id != self.user_id {
                    tx.send(message).await.ok();
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn process_welcome_message(&mut self, data: SessionData<'_>) -> Result<()> {
        self.session_id = Some(data.id.to_string());

        if let Some(url) = data.reconnect_url {
            self.connect_url = url.parse()?;
        }

        let transport = Transport::websocket(&data.id);
        let token = self.token.get(&self.client).await?;

        let subs = self
            .client
            .get_eventsub_subscriptions(None, None, None, &*token)
            .try_collect::<Vec<_>>()
            .await?;

        // Find any active subs for this specific session, so we don't fail on
        // re-creating the event subs that are already in place.
        let subs = subs
            .into_iter()
            .flat_map(|subs| subs.subscriptions)
            .filter_map(|sub| {
                sub.transport
                    .as_websocket()
                    .is_some_and(|ws| ws.session_id == data.id)
                    .then_some(sub.type_)
            })
            .collect::<Vec<_>>();

        trace!(?subs, "loaded active subscriptions");

        if !subs.contains(&EventType::StreamOnline) {
            self.client
                .create_eventsub_subscription(
                    StreamOnlineV1::broadcaster_user_id(self.streamer_id.clone()),
                    transport.clone(),
                    &*token,
                )
                .await?;
        }

        if !subs.contains(&EventType::StreamOffline) {
            self.client
                .create_eventsub_subscription(
                    StreamOfflineV1::broadcaster_user_id(self.streamer_id.clone()),
                    transport.clone(),
                    &*token,
                )
                .await?;
        }

        if !subs.contains(&EventType::ChannelChatMessage) {
            self.client
                .create_eventsub_subscription(
                    ChannelChatMessageV1::new(self.streamer_id.clone(), self.user_id.clone()),
                    transport,
                    &*token,
                )
                .await?;
        }

        Ok(())
    }
}

pub struct Replier {
    streamer_id: UserId,
    user_id: UserId,
    client: HelixClient<'static, reqwest::Client>,
    token: Token,
}

impl Replier {
    pub async fn send_chat_message(&self, msg_id: &MsgId, content: String) -> Result<()> {
        let token = self.token.get(&self.client).await?;
        let resp = self
            .client
            .req_post(
                SendChatMessageRequest::new(),
                SendChatMessageBody::new(&self.streamer_id, &self.user_id, content)
                    .reply_parent_message_id(msg_id),
                &*token,
            )
            .await?;

        ensure!(resp.data.is_sent, "message wasn't sent");

        Ok(())
    }
}

#[derive(Clone)]
struct Token(Arc<Mutex<UserToken>>);

impl Token {
    fn new(token: UserToken) -> Self {
        Self(Arc::new(Mutex::new(token)))
    }

    async fn get(&self, client: &impl Oauth2Client) -> Result<MutexGuard<'_, UserToken>> {
        let mut token = self.0.lock().await;
        if token.expires_in() < Duration::from_secs(120) {
            token
                .refresh_token(client)
                .await
                .context("failed refreshing expired user token")?;
        }

        Ok(token)
    }
}
