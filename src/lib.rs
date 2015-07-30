extern crate sctp_sys;
extern crate libc;

mod sctpsock;
use sctpsock::{SctpSocket, BindOp, RawSocketAddr};
use sctp_sys::SOCK_SEQPACKET;

use std::io::prelude::*;
use std::io::{Result, Error, ErrorKind};
use std::net::{ToSocketAddrs, SocketAddr, Shutdown};

#[cfg(target_os="linux")]
use std::os::unix::io::{AsRawFd, RawFd, FromRawFd};
#[cfg(target_os="windows")]
use std::os::windows::io::{AsRawHandle, RawHandle, FromRawHandle};


/// One-to-one SCTP connected stream which behaves like a TCP stream.
/// A `SctpStream` can be obtained either actively by connecting to a SCTP endpoint with the
/// `connect` constructor, or passively from a `SctpListener` which accepts new connections
pub struct SctpStream(SctpSocket);

impl SctpStream {
	
	/// Create a new stream by connecting it to a remote endpoint
	pub fn connect<A: ToSocketAddrs>(address: A) -> Result<SctpStream> {
		let raw_addr = try!(SocketAddr::from_addr(&address));
		let sock = try!(SctpSocket::new(raw_addr.family(), libc::SOCK_STREAM));
		try!(sock.connect(raw_addr));
		return Ok(SctpStream(sock));
	}
	
	/// Send bytes on the specified SCTP stream. On success, returns the
	/// quantity of bytes read
	pub fn sendmsg(&self, msg: &[u8], stream: u16) -> Result<usize> {
		return self.0.sendmsg::<SocketAddr>(msg, None, stream, 0);
	}
	
	/// Read bytes. On success, return a tulpe with the quantity of
	/// bytes received and the stream they were recived on
	pub fn recvmsg(&self, msg: &mut [u8]) -> Result<(usize, u16)> {
		let (size, stream, _) = try!(self.0.recvmsg(msg));
		return Ok((size, stream));
	}
	
	/// Return the list of local socket addresses for this stream
	pub fn local_addrs(&self) -> Result<Vec<SocketAddr>> {
		return self.0.local_addrs(0);
	}
	
	/// Return the list of socket addresses for the peer this stream is connected to
	pub fn peer_addrs(&self) -> Result<Vec<SocketAddr>> {
		return self.0.peer_addrs(0);
	}
	
	/// Shuts down the read, write, or both halves of this connection
	pub fn shutdown(&self, how: Shutdown) -> Result<()> {
		return self.0.shutdown(how);
	}
	
	/// Set or unset SCTP NO DELAY option
	pub fn set_nodelay(&self, nodelay: bool) -> Result<()> {
		let val: libc::c_int = if nodelay { 1 } else { 0 };
		return self.0.setsockopt(sctp_sys::SCTP_NODELAY, &val);
	}
	
	/// Try to clone the SctpStream. On success, returns a new stream
	/// wrapping a new socket handler
	pub fn try_clone(&self) -> Result<SctpStream> {
		return Ok(SctpStream(try!(self.0.try_clone())));
	}
}

impl Read for SctpStream {
	fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
		return self.0.recv(buf);
	}
}

impl Write for SctpStream {
	fn write(&mut self, buf: &[u8]) -> Result<usize> {
		return self.0.send(buf);
	}
	
	fn flush(&mut self) -> Result<()> {
		return Ok(());
	}
}

#[cfg(target_os="windows")]
impl AsRawHandle for SctpStream {
	fn as_raw_handle(&self) -> RawHandle {
		return return self.0.as_raw_handle();	
	}
}

#[cfg(target_os="windows")]
impl FromRawHandle for SctpStream {
	unsafe fn from_raw_handle(hdl: RawHandle) -> SctpStream {
		return SctpStream(SctpSocket::from_raw_handle(hdl));
	}
}

#[cfg(target_os="linux")]
impl AsRawFd for SctpStream {
	fn as_raw_fd(&self) -> RawFd {
		return self.0.as_raw_fd();	
	}
}

#[cfg(target_os="linux")]
impl FromRawFd for SctpStream {
	unsafe fn from_raw_fd(fd: RawFd) -> SctpStream {
		return SctpStream(SctpSocket::from_raw_fd(fd));
	}
}


/// One-to-many SCTP stream.
pub struct SctpDatagram(SctpSocket);

impl SctpDatagram {
	
	/// Create a one-to-many SCTP socket bound to a single address
	pub fn bind<A: ToSocketAddrs>(address: A) -> Result<SctpDatagram> {
		return Self::bindx(&[address]);
	}
	
	/// Create a one-to-many SCTP socket bound to a multiple addresses. Requires at least one address
	pub fn bindx<A: ToSocketAddrs>(addresses: &[A]) -> Result<SctpDatagram> {
		if addresses.len() == 0 { return Err(Error::new(ErrorKind::InvalidInput, "No addresses given")); }
		let mut vec = Vec::with_capacity(addresses.len());
		let mut family = libc::AF_INET;
		for address in addresses {
			let a = try!(SocketAddr::from_addr(address));
			if a.family() == libc::AF_INET6 { family = libc::AF_INET6; }
			vec.push(a);
		}

		let sock = try!(SctpSocket::new(family, SOCK_SEQPACKET));
		try!(sock.bindx(&vec, BindOp::AddAddr));
		try!(sock.listen(-1));
		return Ok(SctpDatagram(sock));
	}
	
	/// Wait for data to be received. On success, returns a triplet containing
	/// the quantity of bytes received, the sctp stream id on which data were received, and
	/// the socket address used by the peer to send the data
	pub fn recv_from(&self, msg: &mut [u8]) -> Result<(usize, u16, SocketAddr)> {
		return self.0.recvmsg(msg);
	}
	
	/// Send data in Sctp style, to the provided address on the stream `stream`.
	/// On success, returns the quantity on bytes sent
	pub fn send_to<A: ToSocketAddrs>(&self, msg: &mut [u8], address: A, stream: u16, ) -> Result<usize> {
		return self.0.sendmsg(msg, Some(address), stream, 0);
	}
	
	/// Get local socket addresses on which this socket is bound
	pub fn local_addrs(&self) -> Result<Vec<SocketAddr>> {
		return self.0.local_addrs(0);
	}
	
	/// Try to clone this socket
	pub fn try_clone(&self) -> Result<SctpDatagram> {
		return Ok(SctpDatagram(try!(self.0.try_clone())));
	}
}

#[cfg(target_os="windows")]
impl AsRawHandle for SctpDatagram {
	fn as_raw_handle(&self) -> RawHandle {
		return return self.0.as_raw_handle();	
	}
}

#[cfg(target_os="windows")]
impl FromRawHandle for SctpDatagram {
	unsafe fn from_raw_handle(hdl: RawHandle) -> SctpDatagram {
		return SctpDatagram(SctpSocket::from_raw_handle(hdl));
	}
}

#[cfg(target_os="linux")]
impl AsRawFd for SctpDatagram {
	fn as_raw_fd(&self) -> RawFd {
		return self.0.as_raw_fd();	
	}
}

#[cfg(target_os="linux")]
impl FromRawFd for SctpDatagram {
	unsafe fn from_raw_fd(fd: RawFd) -> SctpDatagram {
		return SctpDatagram(SctpSocket::from_raw_fd(fd));
	}
}

/// Iterator over incoming connections on `SctpListener`
pub struct Incoming<'a>(&'a SctpListener);

impl <'a> std::iter::Iterator for Incoming<'a> {
	type Item = Result<SctpStream>;
	
	fn next(&mut self) -> Option<Result<SctpStream>> {
		return match self.0.accept() {
			Ok((stream, _)) => Some(Ok(stream)),
			Err(e) => Some(Err(e))
		};
	}
}


/// SCTP listener which behaves like a `TcpListener`.
/// A SCTP listener is used to wait for and accept one-to-one SCTP connections.
/// An accepted connection is represented by `SctpStream`.
pub struct SctpListener(SctpSocket);

impl SctpListener {
	
	/// Create a listener bound to a single address
	pub fn bind<A: ToSocketAddrs>(address: A) -> Result<SctpListener> {
		return Self::bindx(&[address]);
	}
	
	/// Create a listener bound to multiple addresses. Requires at least one address
	pub fn bindx<A: ToSocketAddrs>(addresses: &[A]) -> Result<SctpListener> {
		if addresses.len() == 0 { return Err(Error::new(ErrorKind::InvalidInput, "No addresses given")); }
		let mut vec = Vec::with_capacity(addresses.len());
		let mut family = libc::AF_INET;
		for address in addresses {
			let a = try!(SocketAddr::from_addr(address));
			if a.family() == libc::AF_INET6 { family = libc::AF_INET6; }
			vec.push(a);
		}

		let sock = try!(SctpSocket::new(family, libc::SOCK_STREAM));
		try!(sock.bindx(&vec, BindOp::AddAddr));
		try!(sock.listen(-1));
		return Ok(SctpListener(sock));
	}
	
	/// Accept a new connection
	pub fn accept(&self) -> Result<(SctpStream, SocketAddr)> {
		let (sock, addr) = try!(self.0.accept());
		return Ok((SctpStream(sock), addr));
	}
	
	/// Iterate over new connections.
	pub fn incoming(&self) -> Incoming {
		return Incoming(self);
	}
	
	/// Get the listener ocal addresses
	pub fn local_addrs(&self) -> Result<Vec<SocketAddr>> {
		return self.0.local_addrs(0);
	}
	
	/// Try to clone this listener
	pub fn try_clone(&self) -> Result<SctpListener> {
		return Ok(SctpListener(try!(self.0.try_clone())));
	}
}

#[cfg(target_os="windows")]
impl AsRawHandle for SctpListener {
	fn as_raw_handle(&self) -> RawHandle {
		return return self.0.as_raw_handle();	
	}
}

#[cfg(target_os="windows")]
impl FromRawHandle for SctpListener {
	unsafe fn from_raw_handle(hdl: RawHandle) -> SctpListener {
		return SctpListener(SctpSocket::from_raw_handle(hdl));
	}
}

#[cfg(target_os="linux")]
impl AsRawFd for SctpListener {
	fn as_raw_fd(&self) -> RawFd {
		return self.0.as_raw_fd();	
	}
}

#[cfg(target_os="linux")]
impl FromRawFd for SctpListener {
	unsafe fn from_raw_fd(fd: RawFd) -> SctpListener {
		return SctpListener(SctpSocket::from_raw_fd(fd));
	}
}
