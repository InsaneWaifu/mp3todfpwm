use std::collections::VecDeque;
use std::convert::Infallible;
use std::future::Future;
use std::{env, io};
use std::net::{SocketAddr, ToSocketAddrs};
use std::process::Stdio;
use futures_util::stream::StreamExt;

use futures_util::SinkExt;
use anyhow::Error;
use log::info;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::process::Command;
use tokio_tungstenite::tungstenite::Message;
use url::Url;


#[tokio::main]
async fn main() -> Result<(), Error> {
    let _ = env_logger::try_init();
    let addr = env::args().nth(1).unwrap_or_else(|| "0.0.0.0:8080".to_string());

    // Create the event loop and TCP listener we'll accept connections on.
    let try_socket = TcpListener::bind(&addr).await;
    let listener = try_socket.expect("Failed to bind");
    info!("Listening on: {}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(accept_connection(stream));
    }

    Ok(())
}

async fn accept_connection(stream: TcpStream) {
    
    let args: Vec<String> = env::args().collect();
    
    let addr = stream.peer_addr().expect("connected streams should have a peer address");
    info!("Peer address: {}", addr);

    let ws_stream = tokio_tungstenite::accept_async(stream)
        .await
        .expect("Error during the websocket handshake occurred");

    info!("New WebSocket connection: {}", addr);

    let url;
    let (mut write, mut read) = ws_stream.split();
    if let Some(x) = read.next().await {
        if let Ok(Message::Text(t)) = x {
            // it seems weird but we will resolve the dns name
            // because ffmpeg doesnt want to on the build i downloaded and i cba to compile it
            let mut ur = Url::parse(&t).unwrap();
            let origport = ur.port();
            let mut new_host_string = ur.host_str().unwrap().to_owned();
            if ur.port().is_none() {
                new_host_string = new_host_string + ":80"
            }
            let hosts = new_host_string.to_socket_addrs().unwrap().collect::<Vec<_>>();
            let sars = hosts.first().unwrap();
            ur.set_host(Some(&sars.ip().to_string())).unwrap();
            ur.set_port(origport).unwrap();
            url = ur.to_string();
        } else {
            return ;
        }
    } else {
        return ;
    }
    let stdoutt;
    if cfg!(target_os = "windows") {
        stdoutt = "pipe:1";
    } else {
        stdoutt = "-";
    }
    let prog;
    if args.len() > 2 {
        prog = args[2].as_str();
    } else {
        prog = "ffmpeg";
    }
    let mut cmd = Command::new(prog).arg("-i").arg(url).arg("-ac").arg("1").arg("-c:a").arg("dfpwm").arg("-ar").arg("48k").arg("-f").arg("dfpwm").arg(stdoutt)
    .stdout(Stdio::piped()).spawn().unwrap();
    
   
    let stdo = cmd.stdout.as_mut().unwrap();
    loop {
        let mut ua = [0u8;16*1024];

        let r = stdo.read_exact(&mut ua).await;
        if r.is_err() {
            break;
        }
        
        let r = write.send(Message::Binary(Vec::from(ua))).await;
        if r.is_err() {
            break;
        }
    }
    cmd.kill().await.unwrap();
    
}