use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc;

enum InternalMessage {
    NewClient(SocketAddr, mpsc::SyncSender<Vec<u8>>),
    ClientDisconnected(SocketAddr),
    NewMessage(Vec<u8>),
}

struct DisconnectNotifier {
    peer_addr: SocketAddr,
    tx: mpsc::Sender<InternalMessage>,
}

impl Drop for DisconnectNotifier {
    fn drop(&mut self) {
        let _ = self.tx.send(InternalMessage::ClientDisconnected(self.peer_addr));
    }
}

fn handle_client_read(mut stream: TcpStream,
                      sender: mpsc::Sender<InternalMessage>,
                      peer_addr: SocketAddr)
                      -> ::std::io::Result<()>
{
    use std::io::Read;

    let _notifer = DisconnectNotifier { peer_addr: peer_addr, tx: sender.clone() };
    loop {
        let mut buf = [0];
        try!(stream.read_exact(&mut buf[..]));
        let mut buf_body = vec![0; buf[0] as usize];
        try!(stream.read_exact(&mut buf_body[..]));
        let _ = sender.send(InternalMessage::NewMessage(buf_body));
    }
}

fn handle_client_write(mut stream: TcpStream, rx: mpsc::Receiver<Vec<u8>>) -> ::std::io::Result<()> {
    use std::io::Write;
    loop {
        match rx.recv() {
            Ok(m) => {
                try!(stream.write_all(&[m.len() as u8]));
                try!(stream.write_all(&m[..]));
            }
            Err(_) => {
                // assume disconnected
                break
            }
        }
    }
    Ok(())
}

fn run() -> Result<(), ::std::io::Error> {

    let args: Vec<String> = ::std::env::args().collect();
    if args.len() != 2 {
        println!("usage: {} HOST:PORT", args[0]);
        return Ok(());
    }

    let addr_str = &args[1];
    let addr = addr_str.parse::<::std::net::SocketAddr>().unwrap();
    let listener = try!(TcpListener::bind(addr));

    let (tx, rx) = mpsc::channel();
    // start "global state" thread.
    ::std::thread::spawn(move || {
        let mut clients =
            ::std::collections::HashMap::<SocketAddr, mpsc::SyncSender<Vec<u8>>>::new();
        loop {
            match rx.recv() {
                Ok(InternalMessage::NewClient(addr, s)) => {
                    clients.insert(addr, s);
                }
                Ok(InternalMessage::ClientDisconnected(addr)) => {
                    clients.remove(&addr);
                }
                Ok(InternalMessage::NewMessage(v)) => {
                    for (_, tx) in &clients {
                        // Try to send without blocking.
                        let _ = tx.try_send(v.clone());
                    }
                }
                Err(_) => {
                    // No senders left, so no more work to do.
                    break;
                }
            }
        }
    });

    println!("listening on {}", addr_str);
    // accept connections and process them, spawning a new thread for each one
    for stream in listener.incoming() {
        let tx = tx.clone();
        match stream {
            Ok(stream) => {
                let (message_tx, message_rx) = mpsc::sync_channel(5);

                let peer_addr = try!(stream.peer_addr());
                let _ = tx.send(InternalMessage::NewClient(peer_addr, message_tx));

                let read_stream = try!(stream.try_clone());

                // start write thread
                ::std::thread::spawn(move|| {
                    let _ = handle_client_write(stream, message_rx);
                });

                // start read thread
                ::std::thread::spawn(move|| {
                    let _ = handle_client_read(read_stream, tx, peer_addr);
                });

            }
            Err(_e) => { /* connection failed */ }
        }
    }

    Ok(())
}

pub fn main() {
    run().expect("top level error")
}
