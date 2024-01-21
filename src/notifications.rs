use matrix_sdk::{
    room::Room as MatrixRoom,
    ruma::{
        api::client::push::get_notifications::v3::Notification,
        events::{room::message::MessageType, AnyMessageLikeEventContent, AnySyncTimelineEvent},
    },
    Client,
};

use crate::{
    base::{IambError, IambResult},
    config::ApplicationSettings,
};

pub async fn register_notifications(client: &Client, settings: &ApplicationSettings) {
    if !settings.tunables.notifications {
        return;
    }
    client
        .register_notification_handler(|notification, room: MatrixRoom, _: Client| {
            async move {
                match parse_notification(notification, room).await {
                    Ok((summary, body)) => {
                        let Some(body) = body else {
                            // Never show without a body.
                            return;
                        };

                        if let Err(err) = std::process::Command::new("tput").arg("bel").status() {
                            tracing::error!("Failed to send bel: {err}")
                        }
                        // TODO: never show if room is currently open.

                        if let Err(err) = notify_rust::Notification::new()
                            .summary(&summary)
                            .body(&body)
                            .appname("iamb")
                            .timeout(notify_rust::Timeout::Milliseconds(3000))
                            .action("default", "default")
                            .show()
                        {
                            tracing::error!("Failed to send notification: {err}")
                        }
                    },
                    Err(err) => {
                        tracing::error!("Failed to extract notification data: {err}")
                    },
                }
            }
        })
        .await;
    return;
}

pub async fn parse_notification(
    notification: Notification,
    room: MatrixRoom,
) -> IambResult<(String, Option<String>)> {
    let event = notification.event.deserialize().map_err(IambError::from)?;

    let sender_id = event.sender();
    let sender = room.get_member_no_sync(sender_id).await.map_err(IambError::from)?;

    let sender_name = sender
        .as_ref()
        .and_then(|m| m.display_name())
        .unwrap_or_else(|| sender_id.localpart());

    let body = event_notification_body(&event, sender_name, room.is_direct());
    return Ok((sender_name.to_string(), body));
}

pub fn event_notification_body(
    event: &AnySyncTimelineEvent,
    sender_name: &str,
    is_direct: bool,
) -> Option<String> {
    let AnySyncTimelineEvent::MessageLike(event) = event else {
        return None;
    };

    match event.original_content()? {
        AnyMessageLikeEventContent::RoomMessage(message) => {
            let body = match message.msgtype {
                MessageType::Audio(_) => {
                    format!("{sender_name} sent an audio file.")
                },
                MessageType::Emote(content) => {
                    let message = &content.body;
                    format!("{sender_name}: {message}")
                },
                MessageType::File(_) => {
                    format!("{sender_name} sent a file.")
                },
                MessageType::Image(_) => {
                    format!("{sender_name} sent an image.")
                },
                MessageType::Location(_) => {
                    format!("{sender_name} sent their location.")
                },
                MessageType::Notice(content) => {
                    let message = &content.body;
                    format!("{sender_name}: {message}")
                },
                MessageType::ServerNotice(content) => {
                    let message = &content.body;
                    format!("{sender_name}: {message}")
                },
                MessageType::Text(content) => {
                    if is_direct {
                        content.body
                    } else {
                        let message = &content.body;
                        format!("{sender_name}: {message}")
                    }
                },
                MessageType::Video(_) => {
                    format!("{sender_name} sent a video.")
                },
                MessageType::VerificationRequest(_) => {
                    format!("{sender_name} sent a verification request.")
                },
                _ => unimplemented!(),
            };
            Some(body)
        },
        AnyMessageLikeEventContent::Sticker(_) => Some(format!("{sender_name} sent a sticker.")),
        _ => None,
    }
}
