use std::thread;
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{channel, Receiver};

pub struct Builder {
}

impl Builder {
  pub fn new() -> Self {
    Self {
    }
  }

  pub fn build(&self) -> Receiver<TcpStream> {
    let (sender, receiver) = channel::<TcpStream>();

    thread::spawn(move || {
      let listener = TcpListener::bind("127.0.0.1:3005").unwrap();
      for stream in listener.incoming() {
        let stream = stream.unwrap();
        
        sender.send(stream).unwrap();
      }
    });

    receiver
  }
}
