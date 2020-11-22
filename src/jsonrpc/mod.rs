mod api;
use api::RpcApi;

use std::{
    io::{self, Read},
    path::PathBuf,
    process,
    time::Duration,
};

#[cfg(not(windows))]
use mio::{
    net::{UnixListener, UnixStream},
    Events, Interest, Poll, Token,
};
#[cfg(windows)]
use uds_windows::{UnixListener, UnixStream};

pub struct RpcImpl;
impl RpcApi for RpcImpl {
    fn stop(&self) -> jsonrpc_core::Result<()> {
        // FIXME: of course, this is Bad :TM:
        process::exit(0);
    }
}

// Remove trailing newlines from utf-8 byte stream
fn trimmed(mut vec: Vec<u8>, bytes_read: usize) -> Vec<u8> {
    vec.truncate(bytes_read);

    // Until there is some whatever-newline character, pop.
    while let Some(byte) = vec.last() {
        // Of course, we assume utf-8
        if byte < &0x0a || byte > &0x0d {
            break;
        }
        vec.pop();
    }

    vec
}

// Returns an error only on a fatal one, and None on recoverable ones.
fn read_bytes_from_stream(mut stream: UnixStream) -> Result<Option<Vec<u8>>, io::Error> {
    let mut buf = vec![0; 512];
    let mut bytes_read = 0;

    loop {
        match stream.read(&mut buf) {
            Ok(0) => return Ok(Some(trimmed(buf, bytes_read))),
            Ok(n) => {
                bytes_read += n;
                if bytes_read == buf.len() {
                    buf.resize(bytes_read * 2, 0);
                } else {
                    return Ok(Some(trimmed(buf, bytes_read)));
                }
            }
            Err(err) => {
                match err.kind() {
                    io::ErrorKind::WouldBlock => {
                        if bytes_read == 0 {
                            // We can't read it just yet, but it's fine.
                            return Ok(None);
                        }
                        return Ok(Some(trimmed(buf, bytes_read)));
                    }
                    io::ErrorKind::Interrupted => {
                        // Try again on interruption.
                        continue;
                    }
                    // Now that's actually bad
                    _ => return Err(err),
                }
            }
        }
    }
}

// Try to parse and interpret bytes from the stream
fn handle_byte_stream(
    jsonrpc_io: &jsonrpc_core::IoHandler,
    stream: UnixStream,
) -> Result<(), io::Error> {
    if let Some(bytes) = read_bytes_from_stream(stream)? {
        match String::from_utf8(bytes) {
            Ok(string) => {
                log::trace!("JSONRPC server: got '{}'", &string);
                // FIXME: couldn't we just spawn it in a thread or handle the future?
                jsonrpc_io.handle_request_sync(&string);
            }
            Err(e) => {
                log::error!(
                    "JSONRPC server: error interpreting request: '{}'",
                    e.to_string()
                );
            }
        }
    }

    Ok(())
}

// For all but Windows, we use Mio.
#[cfg(not(windows))]
fn mio_loop(
    mut listener: UnixListener,
    jsonrpc_io: jsonrpc_core::IoHandler,
) -> Result<(), io::Error> {
    const JSONRPC_SERVER: Token = Token(0);
    let mut poller = Poll::new()?;
    let mut events = Events::with_capacity(16);

    poller
        .registry()
        .register(&mut listener, JSONRPC_SERVER, Interest::READABLE)?;

    loop {
        poller.poll(&mut events, Some(Duration::from_millis(100)))?;

        for event in &events {
            // FIXME: remove, was just out of curiosity
            if event.is_error() {
                log::error!("Got error polling the JSONRPC socket: {:?}", event.token());
            }

            // A connection was established; loop to process all the messages
            if event.token() == JSONRPC_SERVER && event.is_readable() {
                loop {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            handle_byte_stream(&jsonrpc_io, stream)?;
                        }
                        Err(e) => {
                            // Ok; next time then!
                            if e.kind() == io::ErrorKind::WouldBlock {
                                break;
                            }

                            // This one is not expected!
                            return Err(e);
                        }
                    }
                }
            }
        }
    }
}

// For windows, we don't: Mio UDS support for Windows is not yet implemented.
#[cfg(windows)]
fn windows_loop(
    listener: UnixListener,
    jsonrpc_io: jsonrpc_core::IoHandler,
) -> Result<(), io::Error> {
    for stream in listener.incoming() {
        handle_byte_stream(&jsonrpc_io, stream?)?;
    }

    Ok(())
}

/// The main event loop for the JSONRPC interface, polling the UDS at `socket_path`
pub fn jsonrpcapi_loop(socket_path: PathBuf) -> Result<(), io::Error> {
    // FIXME: permissions! (umask before binding ?)
    let listener = UnixListener::bind(&socket_path)?;
    let mut jsonrpc_io = jsonrpc_core::IoHandler::new();
    jsonrpc_io.extend_with(RpcImpl.to_delegate());

    #[cfg(not(windows))]
    return mio_loop(listener, jsonrpc_io);
    #[cfg(windows)]
    return windows_loop(listener, jsonrpc_io);
}
