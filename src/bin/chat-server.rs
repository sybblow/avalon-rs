use std::time::Instant;

use actix::*;
use actix_files as fs;
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;

use avalon_rs::server;
use avalon_rs::session;

/// Entry point for our route
async fn chat_route(
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<server::ChatServer>>,
) -> Result<HttpResponse, Error> {
    ws::start(
        session::WsChatSession {
            id: 0,
            hb: Instant::now(),
            name: None,
            addr: srv.get_ref().clone(),
        },
        &req,
        stream,
    )
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let addr = get_opts();

    // Start chat server actor
    let server = server::ChatServer::default().start();

    // Create Http server with websocket support
    HttpServer::new(move || {
        App::new()
            .data(server.clone())
            // redirect to websocket.html
            .service(web::resource("/").route(web::get().to(|| {
                HttpResponse::Found()
                    .header("LOCATION", "/static/websocket.html")
                    .finish()
            })))
            // websocket
            .service(web::resource("/ws/").to(chat_route))
            // static resources
            .service(fs::Files::new("/static/", "static/"))
    })
    .bind(&addr)?
    .run()
    .await
}

#[inline]
fn get_opts() -> String {
    let matches = clap::App::new("avalon-websocket")
        .version("0.1")
        .author("Cao Siliang <siliang.cao@gmail.com>")
        .about("Web socket server as avalon dealer")
        .arg(
            clap::Arg::with_name("address")
                .short("a")
                .long("addr")
                .help("Sets the listen address")
                .takes_value(true),
        )
        .get_matches();

    matches
        .value_of("address")
        .unwrap_or("127.0.0.1:8080")
        .to_string()
}
