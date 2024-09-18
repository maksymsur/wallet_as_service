use std::convert::TryInto;

use anyhow::{Context, Result};
use futures::{Sink, Stream, StreamExt, TryStreamExt};
use log::{debug, info};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use structopt::StructOpt;

use round_based::Msg;

/// Joins a computation by connecting to a state machine manager.
///
/// This function sets up the communication channels for a party to participate in a multi-party computation.
pub async fn join_computation<M>(
    address: surf::Url,
    room_id: &str,
) -> Result<(
    u16,
    impl Stream<Item = Result<Msg<M>>>,
    impl Sink<Msg<M>, Error = anyhow::Error>,
)>
where
    M: Serialize + DeserializeOwned,
{
    info!(
        "Joining computation. Address: {}, Room ID: {}",
        address, room_id
    );
    let client = SmClient::new(address, room_id).context("Failed to construct SmClient")?;

    // Construct channel of incoming messages
    let incoming = client
        .subscribe()
        .await
        .context("Failed to subscribe")?
        .and_then(|msg| async move {
            serde_json::from_str::<Msg<M>>(&msg).context("Failed to deserialize message")
        });

    // Obtain party index
    let index = client
        .issue_index()
        .await
        .context("Failed to issue an index")?;
    debug!("Obtained party index: {}", index);

    // Ignore incoming messages addressed to someone else
    let incoming = incoming.try_filter(move |msg| {
        futures::future::ready(
            msg.sender != index && (msg.receiver.is_none() || msg.receiver == Some(index)),
        )
    });

    // Construct channel of outgoing messages
    let outgoing = futures::sink::unfold(client, |client, message: Msg<M>| async move {
        let serialized = serde_json::to_string(&message).context("Failed to serialize message")?;
        client
            .broadcast(&serialized)
            .await
            .context("Failed to broadcast message")?;
        Ok::<_, anyhow::Error>(client)
    });

    Ok((index, incoming, outgoing))
}

/// Represents a client for the state machine manager.
pub struct SmClient {
    http_client: surf::Client,
}

impl SmClient {
    /// Creates a new SmClient instance.
    ///
    /// # Arguments
    ///
    /// * `address` - The base URL of the state machine manager.
    /// * `room_id` - The identifier for the computation room.
    pub fn new(address: surf::Url, room_id: &str) -> Result<Self> {
        debug!(
            "Creating new SmClient. Address: {}, Room ID: {}",
            address, room_id
        );
        let config = surf::Config::new()
            .set_base_url(address.join(&format!("rooms/{}/", room_id))?)
            .set_timeout(None);
        Ok(Self {
            http_client: config.try_into()?,
        })
    }

    /// Requests a unique index from the state machine manager.
    pub async fn issue_index(&self) -> Result<u16> {
        debug!("Issuing unique index");
        let response = self
            .http_client
            .post("issue_unique_idx")
            .recv_json::<IssuedUniqueIdx>()
            .await
            .map_err(|e| e.into_inner())?;
        Ok(response.unique_idx)
    }

    /// Broadcasts a message to all parties in the computation.
    pub async fn broadcast(&self, message: &str) -> Result<()> {
        debug!("Broadcasting message");
        self.http_client
            .post("broadcast")
            .body(message)
            .await
            .map_err(|e| e.into_inner())?;
        Ok(())
    }

    /// Subscribes to messages from the state machine manager.
    pub async fn subscribe(&self) -> Result<impl Stream<Item = Result<String>>> {
        info!("Subscribing to messages");
        let response = self
            .http_client
            .get("subscribe")
            .await
            .map_err(|e| e.into_inner())?;
        let events = async_sse::decode(response);
        Ok(events.filter_map(|msg| async {
            match msg {
                Ok(async_sse::Event::Message(msg)) => Some(
                    String::from_utf8(msg.into_bytes())
                        .context("SSE message is not valid UTF-8 string"),
                ),
                Ok(_) => {
                    // Ignore other types of events
                    None
                }
                Err(e) => Some(Err(e.into_inner())),
            }
        }))
    }
}

/// Represents the response from issuing a unique index.
#[derive(Deserialize, Debug)]
struct IssuedUniqueIdx {
    unique_idx: u16,
}

// CLI-related structures and implementations

/// Represents the command-line interface options.
#[derive(StructOpt, Debug)]
struct Cli {
    #[structopt(short, long)]
    address: surf::Url,
    #[structopt(short, long)]
    room: String,
    #[structopt(subcommand)]
    cmd: Cmd,
}

/// Represents the available commands for the CLI.
#[derive(StructOpt, Debug)]
enum Cmd {
    Subscribe,
    Broadcast {
        #[structopt(short, long)]
        message: String,
    },
    IssueIdx,
}

/// Main function for CLI testing purposes.
#[tokio::main]
#[allow(dead_code)]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();

    let args: Cli = Cli::from_args();
    let client = SmClient::new(args.address, &args.room).context("Failed to create SmClient")?;

    match args.cmd {
        Cmd::Broadcast { message } => {
            client
                .broadcast(&message)
                .await
                .context("Failed to broadcast message")?;
        }
        Cmd::IssueIdx => {
            let index = client
                .issue_index()
                .await
                .context("Failed to issue index")?;
            println!("Index: {}", index);
        }
        Cmd::Subscribe => {
            let messages = client.subscribe().await.context("Failed to subscribe")?;
            tokio::pin!(messages);
            while let Some(message) = messages.next().await {
                println!("{:?}", message);
            }
        }
    }

    Ok(())
}
