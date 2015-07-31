extern crate sctp;
use sctp::*;

use std::io::prelude::*;

fn main() {
	// Create a new one-to-one stream
    match SctpStream::connect("127.0.0.1:3868") {
    	Err(e) => println!("{:?}", e.kind()),
    	Ok(mut peer) => {
    		// Set SCTP no delay
    		peer.set_nodelay(true).unwrap();
    		// Write a message using the io::Write trait
    		peer.write_all("foo bar\n".as_bytes()).unwrap();
    		// Write a message on stream 6
    		peer.sendmsg("foo bar again\n".as_bytes(), 6).unwrap();
    		let mut data = [0u8; 1024];
    		// Read data using the io::Read trait
    		peer.read(&mut data).unwrap();
    		// Read data using SCTP advanced feature, and retrieve the stream id
    		// on which data were received
    		let (size, stream) = peer.recvmsg(&mut data).unwrap();
    		println!("Received {} bytes on stream {}", size, stream);
		}
    }
}
