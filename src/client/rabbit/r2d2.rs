use std::io::Error as IoError;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::error::*;
use failure::Compat;
use futures::future::Either;
use lapin_async::connection::ConnectionState;
use lapin_futures::channel::{Channel, ConfirmSelectOptions};
use lapin_futures::client::{Client, ConnectionOptions, HeartbeatHandle};
use prelude::*;
use r2d2::ManageConnection;
use tokio;
use tokio::net::tcp::TcpStream;
use tokio::timer::timeout::Timeout;

pub struct RabbitConnectionManager {
    client: Arc<Mutex<Client<TcpStream>>>,
    heartbeat_handle: Arc<Mutex<RabbitHeartbeatHandle>>,
    connection_timeout: Duration,
    address: SocketAddr,
}

struct RabbitHeartbeatHandle(Option<HeartbeatHandle>);

impl RabbitHeartbeatHandle {
    pub fn new(handle: HeartbeatHandle) -> Self {
        RabbitHeartbeatHandle(Some(handle))
    }
}

impl Drop for RabbitHeartbeatHandle {
    fn drop(&mut self) {
        let handle = self.0.take();
        if let Some(h) = handle {
            h.stop();
        }
    }
}

impl RabbitConnectionManager {
    pub fn establish(address: SocketAddr, connection_timeout: Duration) -> impl Future<Item = Self, Error = Error> {
        let address_clone = address.clone();
        Timeout::new(
            RabbitConnectionManager::establish_client(address).map(move |(client, hearbeat_handle)| RabbitConnectionManager {
                client: Arc::new(Mutex::new(client)),
                heartbeat_handle: Arc::new(Mutex::new(hearbeat_handle)),
                connection_timeout,
                address,
            }),
            connection_timeout,
        ).map_err(
            move |_| ectx!(err ErrorSource::Timeout, ErrorContext::ConnectionTimeout, ErrorKind::Internal => address_clone, connection_timeout),
        )
    }

    fn repair(&self) -> impl Future<Item = (), Error = Error> {
        if self.is_connecting_conn() {
            return Either::A(Err(ectx!(err ErrorContext::AlreadyConnecting, ErrorKind::Internal)).into_future());
        }
        let self_client = self.client.clone();
        let self_hearbeat_handle = self.heartbeat_handle.clone();
        Either::B(
            RabbitConnectionManager::establish_client(self.address).map(move |(client, hearbeat_handle)| {
                {
                    let mut self_client = self_client.lock().unwrap();
                    *self_client = client;
                }
                {
                    let mut self_hearbeat_handle = self_hearbeat_handle.lock().unwrap();
                    *self_hearbeat_handle = hearbeat_handle;
                }
            }),
        )
    }

    fn establish_client(address: SocketAddr) -> impl Future<Item = (Client<TcpStream>, RabbitHeartbeatHandle), Error = Error> {
        let address_clone = address.clone();
        let address_clone2 = address.clone();
        let address_clone3 = address.clone();
        TcpStream::connect(&address)
            .map_err(ectx!(ErrorSource::Io, ErrorContext::TcpConnection, ErrorKind::Internal => address_clone3))
            .and_then(move |stream| {
                Client::connect(
                    stream,
                    ConnectionOptions {
                        frame_max: 65535,
                        ..Default::default()
                    },
                ).map_err(ectx!(ErrorSource::Io, ErrorContext::RabbitConnection, ErrorKind::Internal => address_clone2))
            }).and_then(move |(client, mut heartbeat)| {
                let handle = heartbeat.handle();
                tokio::spawn(heartbeat.map_err(|e| error!("{:?}", e)));
                handle
                    .ok_or(ectx!(err ErrorContext::HeartbeatHandle, ErrorKind::Internal))
                    .map(move |handle| (client, RabbitHeartbeatHandle::new(handle)))
            })
    }

    fn is_broken_conn(&self) -> bool {
        let cli = self.client.lock().unwrap();
        let transport = cli.transport.lock().unwrap();
        match transport.conn.state {
            ConnectionState::Closing(_) | ConnectionState::Closed | ConnectionState::Error => true,
            _ => false,
        }
    }

    fn is_connecting_conn(&self) -> bool {
        let cli = self.client.lock().unwrap();
        let transport = cli.transport.lock().unwrap();
        match transport.conn.state {
            ConnectionState::Connecting(_) | ConnectionState::Connected => true,
            _ => false,
        }
    }

    fn is_connected_chan(&self, chan: &Channel<TcpStream>) -> bool {
        let cli = self.client.lock().unwrap();
        let transport = cli.transport.lock().unwrap();
        transport.conn.is_connected(chan.id)
    }
}

impl ManageConnection for RabbitConnectionManager {
    type Connection = Channel<TcpStream>;
    type Error = Compat<Error>;
    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let cli = self.client.lock().unwrap();
        cli.create_confirm_channel(ConfirmSelectOptions { nowait: false })
            .wait()
            .map_err(ectx!(ErrorSource::Io, ErrorContext::RabbitChannel, ErrorKind::Internal))
            .map_err(|e: Error| e.compat())
    }
    fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        if self.is_broken_conn() {
            let e: Error = ectx!(err format_err!("Connection is broken"), ErrorKind::Internal);
            return Err(e.compat());
        }
        if self.is_connecting_conn() {
            let e: Error = ectx!(err format_err!("Connection is in process of connecting"), ErrorKind::Internal);
            return Err(e.compat());
        }
        if self.is_connected_chan(conn) {
            let e: Error = ectx!(err format_err!("Channel is not connected"), ErrorKind::Internal);
            return Err(e.compat());
        }
        Ok(())
    }
    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        self.is_valid(conn).is_err()
    }
}
