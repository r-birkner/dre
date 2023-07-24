use core::time;
use std::{
    fmt::{self, Display},
    sync::mpsc::Receiver,
};

use actix_web::rt::time::sleep;
use ic_management_types::Status;
use ic_types::PrincipalId;
use slog::Logger;
use tokio_util::sync::CancellationToken;

use crate::matrix;

pub struct NotificationSenderLoopConfig {
    pub logger: Logger,
    pub notification_receiver: Receiver<Notification>,
    pub cancellation_token: CancellationToken,
}

pub async fn start_notification_sender_loop(config: NotificationSenderLoopConfig, sinks: Vec<Sink>) {
    debug!(config.logger, "Starting notification sender loop");
    loop {
        if config.cancellation_token.is_cancelled() {
            break;
        }
        while let Ok(notification) = config.notification_receiver.try_recv() {
            let notif = notification.clone();
            for sink in sinks.iter() {
                let _ = sink.send(notif.clone()).await;
            }
        }
        sleep(time::Duration::from_secs(1)).await;
    }
}

#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Clone)]
pub struct Notification {
    pub node_id: PrincipalId,
    pub status_change: (Status, Status),
}

impl Display for Notification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Node {} changed status from {} to {}",
            self.node_id, self.status_change.0, self.status_change.1
        )
    }
}

#[derive(Debug)]
pub enum SinkError {
    PublicationError,
}

pub enum Sink {
    Log(LogSink),
    Matrix(MatrixSink),
}

impl Sink {
    async fn send(&self, notification: Notification) -> Result<(), SinkError> {
        match self {
            Sink::Log(sink) => sink.send(notification),
            Sink::Matrix(sink) => sink.send(notification).await,
        }
    }
}

#[derive(Clone)]
pub struct LogSink {
    pub logger: Logger,
}

impl LogSink {
    fn send(&self, notification: Notification) -> Result<(), SinkError> {
        info!(self.logger, ""; "sink" => "log", "node_id" => ?notification.node_id, "status_change" => ?notification.status_change);
        Ok(())
    }
}

pub struct MatrixSink {
    pub logger: Logger,
    pub matrix_client: matrix::Client,
}

impl MatrixSink {
    async fn send(&self, notification: Notification) -> Result<(), SinkError> {
        self.matrix_client
            .send_message_to_room("!jeoHAONXXskUWAPpKH:matrix.org".to_string(), notification.to_string())
            .await
            .map_err(|_| SinkError::PublicationError)
            .map(|_| ())
    }
}
