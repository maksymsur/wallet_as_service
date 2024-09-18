use std::collections::hash_map::{Entry, HashMap};
use std::sync::{
    atomic::{AtomicU16, Ordering},
    Arc,
};

use log::{debug, error, info, warn};
use rocket::data::ToByteUnit;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use rocket::response::stream::{Event, EventStream};
use rocket::serde::json::Json;
use rocket::State;
use serde::{Deserialize, Serialize};
use tokio::sync::{Notify, RwLock};

// Constants for configuration
const MAX_MESSAGE_SIZE: u64 = 100 * 1024 * 1024; // 100 MB

/// Handles subscription requests for a specific room
#[rocket::get("/rooms/<room_id>/subscribe")]
async fn subscribe<'a>(
    db: &'a State<Db>,
    mut shutdown: rocket::Shutdown,
    last_seen_msg: LastEventId,
    room_id: &'a str,
) -> EventStream![Event + 'a] {
    let room = db.get_room_or_create_empty(room_id).await;
    let mut subscription = room.subscribe(last_seen_msg.0);
    info!("New subscription for room: {}", room_id);

    EventStream! {
        loop {
            let (id, msg) = tokio::select! {
                message = subscription.next() => message,
                _ = &mut shutdown => {
                    info!("Shutting down subscription for room: {}", room_id);
                    break;
                },
            };
            yield Event::data(msg)
                .event("new-message")
                .id(id.to_string());
        }
    }
}

/// Issues a unique index for a room
#[rocket::post("/rooms/<room_id>/issue_unique_idx")]
async fn issue_idx(db: &State<Db>, room_id: &str) -> Json<IssuedUniqueIdx> {
    let room = db.get_room_or_create_empty(room_id).await;
    let idx = room.issue_unique_idx();
    info!("Issued unique index {} for room: {}", idx, room_id);
    Json::from(IssuedUniqueIdx { unique_idx: idx })
}

/// Broadcasts a message to a specific room
#[rocket::post("/rooms/<room_id>/broadcast", data = "<message>")]
async fn broadcast(db: &State<Db>, room_id: &str, message: String) -> Status {
    let room = db.get_room_or_create_empty(room_id).await;
    room.publish(message).await;
    info!("Broadcasted message to room: {}", room_id);
    Status::Ok
}

/// Represents the database of rooms
struct Db {
    rooms: RwLock<HashMap<String, Arc<Room>>>,
}

impl Db {
    /// Creates an empty database
    pub fn empty() -> Self {
        Self {
            rooms: RwLock::new(HashMap::new()),
        }
    }

    /// Gets an existing room or creates a new one if it doesn't exist
    pub async fn get_room_or_create_empty(&self, room_id: &str) -> Arc<Room> {
        let rooms = self.rooms.read().await;
        if let Some(room) = rooms.get(room_id) {
            if !room.is_abandoned() {
                return room.clone();
            }
        }
        drop(rooms);

        let mut rooms = self.rooms.write().await;
        match rooms.entry(room_id.to_owned()) {
            Entry::Occupied(entry) if !entry.get().is_abandoned() => entry.get().clone(),
            Entry::Occupied(entry) => {
                debug!("Cleaning up abandoned room: {}", room_id);
                let room = Arc::new(Room::empty());
                *entry.into_mut() = room.clone();
                room
            }
            Entry::Vacant(entry) => {
                debug!("Creating new room: {}", room_id);
                entry.insert(Arc::new(Room::empty())).clone()
            }
        }
    }
}

/// Represents a room where clients can subscribe and broadcast messages
struct Room {
    messages: RwLock<Vec<String>>,
    message_appeared: Notify,
    subscribers: AtomicU16,
    next_idx: AtomicU16,
}

impl Room {
    /// Creates an empty room
    pub fn empty() -> Self {
        Self {
            messages: RwLock::new(vec![]),
            message_appeared: Notify::new(),
            subscribers: AtomicU16::new(0),
            next_idx: AtomicU16::new(1),
        }
    }

    /// Publishes a new message to the room
    pub async fn publish(self: &Arc<Self>, message: String) {
        let mut messages = self.messages.write().await;
        messages.push(message);
        self.message_appeared.notify_waiters();
    }

    /// Creates a new subscription to the room
    pub fn subscribe(self: Arc<Self>, last_seen_msg: Option<u16>) -> Subscription {
        let subscribers = self.subscribers.fetch_add(1, Ordering::SeqCst);
        debug!(
            "New subscriber joined. Total subscribers: {}",
            subscribers + 1
        );
        Subscription {
            room: self,
            next_event: last_seen_msg.map(|i| i + 1).unwrap_or(0),
        }
    }

    /// Checks if the room is abandoned (has no subscribers)
    pub fn is_abandoned(&self) -> bool {
        self.subscribers.load(Ordering::SeqCst) == 0
    }

    /// Issues a unique index for the room
    pub fn issue_unique_idx(&self) -> u16 {
        self.next_idx.fetch_add(1, Ordering::Relaxed)
    }
}

/// Represents a subscription to a room
struct Subscription {
    room: Arc<Room>,
    next_event: u16,
}

impl Subscription {
    /// Gets the next message in the subscription
    pub async fn next(&mut self) -> (u16, String) {
        loop {
            let history = self.room.messages.read().await;
            if let Some(msg) = history.get(usize::from(self.next_event)) {
                let event_id = self.next_event;
                self.next_event = event_id + 1;
                return (event_id, msg.clone());
            }
            let notification = self.room.message_appeared.notified();
            drop(history);
            notification.await;
        }
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        let subscribers = self.room.subscribers.fetch_sub(1, Ordering::SeqCst);
        debug!("Subscriber left. Total subscribers: {}", subscribers - 1);
    }
}

/// Represents the Last-Event-ID header
struct LastEventId(Option<u16>);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for LastEventId {
    type Error = &'static str;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let header = request
            .headers()
            .get_one("Last-Event-ID")
            .map(|id| id.parse::<u16>());
        match header {
            Some(Ok(last_seen_msg)) => Outcome::Success(LastEventId(Some(last_seen_msg))),
            Some(Err(_parse_err)) => {
                warn!("Invalid Last-Event-ID header received");
                Outcome::Error((Status::BadRequest, "last seen msg id is not valid"))
            }
            None => Outcome::Success(LastEventId(None)),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct IssuedUniqueIdx {
    unique_idx: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    info!("Starting gg20_sm_manager server");

    let figment = rocket::Config::figment().merge((
        "limits",
        rocket::data::Limits::new().limit("string", MAX_MESSAGE_SIZE.bytes()),
    ));

    let result = rocket::custom(figment)
        .mount("/", rocket::routes![subscribe, issue_idx, broadcast])
        .manage(Db::empty())
        .launch()
        .await;

    match result {
        Ok(_) => info!("Server shutdown successfully"),
        Err(e) => error!("Server error: {:?}", e),
    }

    Ok(())
}
