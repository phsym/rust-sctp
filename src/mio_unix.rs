#[macro_export]
// copy from mio::sys::unix::mod
// https://github.com/faern/mio/blob/master/src/sys/unix/mod.rs
/// Helper macro to execute a system call that returns an `io::Result`.
//
// Macro must be defined before any modules that uses them.
#[allow(unused_macros)]
macro_rules! syscall {
	($fn: ident ( $($arg: expr),* $(,)* ) ) => {{
		let res = unsafe { libc::$fn($($arg, )*) };
		if res == -1 {
			Err(std::io::Error::last_os_error())
		} else {
			Ok(res)
		}
	}};
}

#[macro_export]
#[allow(unused_macros)]
macro_rules! sctp_syscall {
	($fn: ident ( $($arg: expr),* $(,)* ) ) => {{
		let res = unsafe { sctp_sys::$fn($($arg, )*) };
		if res == -1 {
			Err(std::io::Error::last_os_error())
		} else {
			Ok(res)
		}
	}};
}

use std::mem;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

// copy from mio::sys::unix::net
// https://github.com/faern/mio/blob/master/src/sys/unix/net.rs
/// A type with the same memory layout as `libc::sockaddr`. Used in converting Rust level
/// SocketAddr* types into their system representation. The benefit of this specific
/// type over using `libc::sockaddr_storage` is that this type is exactly as large as it
/// needs to be and not a lot larger. And it can be initialized cleaner from Rust.
#[repr(C)]
pub(crate) union SocketAddrCRepr {
    v4: libc::sockaddr_in,
    v6: libc::sockaddr_in6,
}

impl SocketAddrCRepr {
    pub(crate) fn as_ptr(&self) -> *const libc::sockaddr {
        self as *const _ as *const libc::sockaddr
    }
}

/// Converts a Rust `SocketAddr` into the system representation.
pub(crate) fn socket_addr(addr: &SocketAddr) -> (SocketAddrCRepr, libc::socklen_t) {
    match addr {
        SocketAddr::V4(ref addr) => {
            // `s_addr` is stored as BE on all machine and the array is in BE order.
            // So the native endian conversion method is used so that it's never swapped.
            let sin_addr = libc::in_addr {
                s_addr: u32::from_ne_bytes(addr.ip().octets()),
            };

            let sockaddr_in = libc::sockaddr_in {
                sin_family: libc::AF_INET as libc::sa_family_t,
                sin_port: addr.port().to_be(),
                sin_addr,
                sin_zero: [0; 8],
                #[cfg(any(
                    target_os = "dragonfly",
                    target_os = "freebsd",
                    target_os = "ios",
                    target_os = "macos",
                    target_os = "netbsd",
                    target_os = "openbsd"
                ))]
                sin_len: 0,
            };

            let sockaddr = SocketAddrCRepr { v4: sockaddr_in };
            (
                sockaddr,
                mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
            )
        }
        SocketAddr::V6(ref addr) => {
            let sockaddr_in6 = libc::sockaddr_in6 {
                sin6_family: libc::AF_INET6 as libc::sa_family_t,
                sin6_port: addr.port().to_be(),
                sin6_addr: libc::in6_addr {
                    s6_addr: addr.ip().octets(),
                },
                sin6_flowinfo: addr.flowinfo(),
                sin6_scope_id: addr.scope_id(),
                #[cfg(any(
                    target_os = "dragonfly",
                    target_os = "freebsd",
                    target_os = "ios",
                    target_os = "macos",
                    target_os = "netbsd",
                    target_os = "openbsd"
                ))]
                sin6_len: 0,
                #[cfg(any(target_os = "solaris", target_os = "illumos"))]
                __sin6_src_id: 0,
            };

            let sockaddr = SocketAddrCRepr { v6: sockaddr_in6 };
            (
                sockaddr,
                mem::size_of::<libc::sockaddr_in6>() as libc::socklen_t,
            )
        }
    }
}

/// Converts a `libc::sockaddr` compatible struct into a native Rust `SocketAddr`.
///
/// # Safety
///
/// `storage` must have the `ss_family` field correctly initialized.
/// `storage` must be initialised to a `sockaddr_in` or `sockaddr_in6`.
pub(crate) unsafe fn to_socket_addr(
    storage: *const libc::sockaddr_storage,
) -> std::io::Result<SocketAddr> {
    match (*storage).ss_family as libc::c_int {
        libc::AF_INET => {
            // Safety: if the ss_family field is AF_INET then storage must be a sockaddr_in.
            let addr: &libc::sockaddr_in = &*(storage as *const libc::sockaddr_in);
            let ip = Ipv4Addr::from(addr.sin_addr.s_addr.to_ne_bytes());
            let port = u16::from_be(addr.sin_port);
            Ok(SocketAddr::V4(SocketAddrV4::new(ip, port)))
        }
        libc::AF_INET6 => {
            // Safety: if the ss_family field is AF_INET6 then storage must be a sockaddr_in6.
            let addr: &libc::sockaddr_in6 = &*(storage as *const libc::sockaddr_in6);
            let ip = Ipv6Addr::from(addr.sin6_addr.s6_addr);
            let port = u16::from_be(addr.sin6_port);
            Ok(SocketAddr::V6(SocketAddrV6::new(
                ip,
                port,
                addr.sin6_flowinfo,
                addr.sin6_scope_id,
            )))
        }
        _ => Err(std::io::ErrorKind::InvalidInput.into()),
    }
}
//
// CODE COPY ENDS HERE
//
