use std::time::{Duration, Instant};

use actix::*;
use actix_web_actors::ws;
use log::*;

use crate::game;
use crate::server;

/// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

pub struct WsChatSession {
    /// unique session id
    pub id: usize,
    /// Client must send ping at least once per 10 seconds (CLIENT_TIMEOUT),
    /// otherwise we drop connection.
    pub hb: Instant,
    /// peer name
    pub name: Option<String>,
    /// Chat server
    pub addr: Addr<server::ChatServer>,
}

impl Actor for WsChatSession {
    type Context = ws::WebsocketContext<Self>;

    /// Method is called on actor start.
    /// We register ws session with ChatServer
    fn started(&mut self, ctx: &mut Self::Context) {
        // we'll start heartbeat process on session start.
        self.hb(ctx);

        // register self in chat server. `AsyncContext::wait` register
        // future within context, but context waits until this future resolves
        // before processing any other events.
        // HttpContext::state() is instance of WsChatSessionState, state is shared
        // across all routes within application
        let addr = ctx.address();
        self.addr
            .send(server::Connect {
                addr: addr.recipient(),
            })
            .into_actor(self)
            .then(|res, act, ctx| {
                match res {
                    Ok(res) => act.id = res,
                    // something is wrong with chat server
                    _ => ctx.stop(),
                }
                fut::ready(())
            })
            .wait(ctx);
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        // notify chat server
        self.addr.do_send(server::Disconnect { id: self.id });
        Running::Stop
    }
}

/// Handle messages from chat server, we simply send it to peer websocket
impl Handler<server::Message> for WsChatSession {
    type Result = ();

    fn handle(&mut self, msg: server::Message, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

/// WebSocket message handler
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsChatSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        let msg = match msg {
            Err(_) => {
                ctx.stop();
                return;
            }
            Ok(msg) => msg,
        };

        debug!("WEBSOCKET MESSAGE: {:?}", msg);
        match msg {
            ws::Message::Ping(msg) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            ws::Message::Pong(_) => {
                self.hb = Instant::now();
            }
            ws::Message::Text(text) => {
                let m = text.trim();
                // we check for /sss type of messages
                if m.starts_with('/') {
                    let v: Vec<&str> = m.splitn(2, ' ').collect();
                    match v[0] {
                        "/list" => {
                            // Send ListRooms message to chat server and wait for
                            // response
                            info!("List rooms");
                            self.addr
                                .send(server::ListRooms)
                                .into_actor(self)
                                .then(|res, _, ctx| {
                                    match res {
                                        Ok(rooms) => {
                                            for room in rooms {
                                                ctx.text(room);
                                            }
                                        }
                                        _ => warn!("Something is wrong"),
                                    }
                                    fut::ready(())
                                })
                                .wait(ctx)
                            // .wait(ctx) pauses all events in context,
                            // so actor wont receive any new messages until it get list
                            // of rooms back
                        }
                        "/join" => {
                            match (self.name.as_ref(), &v[1..]) {
                                (Some(session_name), [name]) => {
                                    self.addr.do_send(server::Join {
                                        id: self.id,
                                        name: name.to_string(),
                                        session_name: session_name.clone(),
                                    });
                                }
                                (None, _) => {
                                    ctx.text("!!! session name is required");
                                }
                                (Some(_), []) => {
                                    ctx.text("!!! room name is required");
                                }
                                _ => {
                                    ctx.text("!!! unknown command");
                                }
                            };
                        }
                        "/create" => {
                            match (self.name.as_ref(), &v[1..]) {
                                (Some(session_name), [size]) => {
                                    if let Ok(size) = size.parse::<u8>() {
                                        if (size as usize) >= game::LOWER_ROOM_SIZE
                                            && (size as usize) <= game::UPPER_ROOM_SIZE
                                        {
                                            self.addr.do_send(server::Create {
                                                id: self.id,
                                                size,
                                                session_name: session_name.clone(),
                                            });
                                        } else {
                                            ctx.text(format!(
                                                "!!! room size {} is not supported. it should be in range {}-{}",
                                                size, game::LOWER_ROOM_SIZE, game::UPPER_ROOM_SIZE,
                                            ));
                                        }
                                    } else {
                                        ctx.text(format!("!!! invalid room size: {}", size));
                                    }
                                }
                                (None, _) => {
                                    ctx.text("!!! session name is required");
                                }
                                (Some(_), []) => {
                                    ctx.text("!!! size is required");
                                }
                                _ => {
                                    ctx.text("!!! unknown command");
                                }
                            };
                        }
                        "/name" => match &v[1..] {
                            [name] => {
                                self.name = Some(name.to_string());
                            }
                            [] => {
                                ctx.text("!!! name is required");
                            }
                            _ => {
                                ctx.text("!!! unknown command");
                            }
                        },
                        _ => ctx.text(format!("!!! unknown command: {:?}", m)),
                    }
                } else {
                    ctx.text(format!("!!! unknown command: {:?}", m))
                }
            }
            ws::Message::Binary(_) => warn!("Unexpected binary"),
            ws::Message::Close(_) => {
                ctx.stop();
            }
            ws::Message::Continuation(_) => {
                ctx.stop();
            }
            ws::Message::Nop => (),
        }
    }
}

impl WsChatSession {
    /// helper method that sends ping to client every second.
    ///
    /// also this method checks heartbeats from client
    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // check client heartbeats
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                // heartbeat timed out
                debug!("Websocket Client heartbeat failed, disconnecting!");

                // notify chat server
                act.addr.do_send(server::Disconnect { id: act.id });

                // stop actor
                ctx.stop();

                // don't try to send a ping
                return;
            }

            ctx.ping(b"");
        });
    }
}
