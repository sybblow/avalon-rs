//! `ChatServer` is an actor. It maintains list of connection client session.
//! And manages available rooms. Peers send messages to other peers in same
//! room through `ChatServer`.

use std::collections::{BTreeMap, BTreeSet};
use std::iter::Iterator;

use actix::prelude::*;
use failure::Error;
use log::*;
use rand::{self, rngs::ThreadRng, Rng};

use crate::game::Assignment;

/// Chat server sends this messages to session
#[derive(Message)]
#[rtype(result = "()")]
pub struct Message(pub String);

/// Message for chat server communications

/// New chat session is created
#[derive(Message)]
#[rtype(usize)]
pub struct Connect {
    pub addr: Recipient<Message>,
}

/// Session is disconnected
#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub id: usize,
}

/// List of available rooms
pub struct ListRooms;

impl actix::Message for ListRooms {
    type Result = Vec<String>;
}

/// Join room, room must exist.
#[derive(Message)]
#[rtype(result = "()")]
pub struct Join {
    /// Client id
    pub id: usize,
    /// Client name
    pub session_name: String,
    /// Room name
    pub name: String,
}

/// Create room, create and join a new room.
#[derive(Message)]
#[rtype(result = "()")]
pub struct Create {
    /// Client id
    pub id: usize,
    /// Client name
    pub session_name: String,
    /// Room size
    pub size: u8,
}

/// `ChatServer` manages chat rooms and responsible for coordinating chat
/// session. implementation is super primitive
pub struct ChatServer {
    sessions: BTreeMap<usize, Recipient<Message>>,
    rooms: BTreeMap<String, Room>,
    rng: ThreadRng,
}

pub struct Room {
    sessions: BTreeSet<usize>,
    /// Room size
    size: u8,
    /// Client id and name pair list
    seats: Vec<(usize, String)>,
}

impl Room {
    fn is_full(&self) -> bool {
        return self.sessions.len() == self.size as usize;
    }
}

impl Default for ChatServer {
    fn default() -> ChatServer {
        // default room
        let rooms = BTreeMap::new();

        ChatServer {
            sessions: BTreeMap::new(),
            rooms,
            rng: rand::thread_rng(),
        }
    }
}

impl ChatServer {
    /// Send message to all users in the room
    fn broadcast_message(&self, room: &str, message: &str, skip_id: usize) {
        if let Some(Room { sessions, .. }) = self.rooms.get(room) {
            for id in sessions {
                if *id != skip_id {
                    if let Some(addr) = self.sessions.get(id) {
                        let _ = addr.do_send(Message(message.to_owned()));
                    }
                }
            }
        }
    }

    /// Send message to a specified user in the room
    fn send_message_to_user(&self, id: usize, message: String) {
        if let Some(addr) = self.sessions.get(&id) {
            let _ = addr.do_send(Message(message));
        }
    }

    fn assign_and_notify(&self, room: &str) -> Result<(), Error> {
        if let Some(Room { ref seats, .. }) = self.rooms.get(room) {
            let assignment = Assignment::new(seats.iter().map(|(_, name)| name.clone()))?;

            for (seat_no, &(_, role)) in assignment.players.iter().enumerate() {
                let id = seats[seat_no].0;
                self.send_message_to_user(id, format!("你的身份是【{}】，", role));
                let assignment_text = assignment.see_from_role(role).text_from_player(seat_no);
                if assignment_text.is_empty() {
                    self.send_message_to_user(id, format!("你没有提示"));
                } else {
                    self.send_message_to_user(id, assignment_text);
                }
            }
        }

        Ok(())
    }

    fn remove_user_from_all_rooms(&mut self, id: usize) {
        let mut removed_rooms: Vec<String> = Vec::new();
        let mut empty_rooms: Vec<String> = Vec::new();
        // remove session from all rooms
        for (
            name,
            Room {
                ref mut sessions,
                ref mut seats,
                ..
            },
        ) in &mut self.rooms
        {
            if sessions.remove(&id) {
                removed_rooms.push(name.to_owned());

                seats.retain(|&(session_id, _)| session_id != id);

                // more cautious, in case of new created rooms
                if sessions.is_empty() {
                    empty_rooms.push(name.to_owned());
                }
            }
        }
        // clean empty rooms
        for room in empty_rooms {
            self.rooms.remove(&room);
        }
        // send message to other users
        for room in removed_rooms {
            self.broadcast_message(&room, "Someone disconnected", 0);
        }
    }

    //
    //    /// Send message to all users in the room
    //    fn send_message_to_all(&self, room: &str, message: &str) {
    //        if let Some(sessions) = self.rooms.get(room) {
    //            for id in sessions {
    //                if let Some(addr) = self.sessions.get(id) {
    //                    let _ = addr.do_send(Message(message.to_owned()));
    //                }
    //            }
    //        }
    //    }
}

/// Make actor from `ChatServer`
impl Actor for ChatServer {
    /// We are going to use simple Context, we just need ability to communicate
    /// with other actors.
    type Context = Context<Self>;
}

/// Handler for Connect message.
///
/// Register new session and assign unique id to this session
impl Handler<Connect> for ChatServer {
    type Result = usize;

    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) -> Self::Result {
        debug!("Someone joined");

        // register session with random id
        let id = self.rng.gen::<usize>();
        self.sessions.insert(id, msg.addr);

        // send id back
        id
    }
}

/// Handler for Disconnect message.
impl Handler<Disconnect> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        debug!("Someone disconnected");

        // remove address
        if self.sessions.remove(&msg.id).is_some() {
            self.remove_user_from_all_rooms(msg.id)
        }
    }
}

/// Handler for `ListRooms` message.
impl Handler<ListRooms> for ChatServer {
    type Result = MessageResult<ListRooms>;

    fn handle(&mut self, _: ListRooms, _: &mut Context<Self>) -> Self::Result {
        let mut rooms = Vec::new();

        for key in self.rooms.keys() {
            rooms.push(key.to_owned())
        }

        MessageResult(rooms)
    }
}

/// Join room, send disconnect message to old room
/// send join message to new room
impl Handler<Join> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Join, _: &mut Context<Self>) {
        let Join {
            id,
            session_name,
            name,
        } = msg;

        if !self.rooms.contains_key(&name) {
            self.send_message_to_user(id, format!("!!! room not exist"));
            return;
        }

        self.remove_user_from_all_rooms(id);

        let is_full = match self.rooms.get_mut(&name) {
            Some(room) => {
                room.sessions.insert(id);
                // FIXME: check duplicated name
                room.seats.push((id, session_name.clone()));

                room.is_full()
            }
            None => {
                self.send_message_to_user(
                    id,
                    format!("!!! room not exist, may be deleted just now"),
                );
                return;
            }
        };

        self.broadcast_message(&name, &format!("{} connected", &session_name), id);
        self.send_message_to_user(id, format!("joined"));
        if is_full {
            self.broadcast_message(&name, "人已经凑齐", 0);
            if let Err(err) = self.assign_and_notify(&name) {
                self.broadcast_message(&name, &format!("分配失败：{}", err), 0);
            }
            self.rooms.remove(&name);
        }
    }
}

/// Create, send disconnect message to old room
/// send join message to new room
impl Handler<Create> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Create, _: &mut Context<Self>) {
        let Create {
            id,
            session_name,
            size,
        } = msg;
        let name: u32 = self.rng.gen_range(0, 1000);
        let name = name.to_string();
        if self.rooms.contains_key(&name) {
            // TODO: better random number
            self.send_message_to_user(id, "!!! create room failed".to_owned());
            return;
        }

        self.remove_user_from_all_rooms(id);

        self.send_message_to_user(id, format!("room {} created.", &name));
        self.send_message_to_user(id, "请把房间号告诉你的小伙伴们".to_owned());
        let mut sessions = BTreeSet::new();
        sessions.insert(id);
        let seats = vec![(id, session_name)];
        self.rooms.insert(
            name.clone(),
            Room {
                sessions,
                size,
                seats,
            },
        );
    }
}
