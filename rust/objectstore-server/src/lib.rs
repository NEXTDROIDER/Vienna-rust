use std::future::Future;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use thiserror::Error;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufStream};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum DataStoreError {
    #[error("data root directory is not readable: {0}")]
    InvalidRoot(PathBuf),
    #[error("failed to persist object data")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Error)]
pub enum ObjectStoreError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    DataStore(#[from] DataStoreError),
    #[error("malformed object store protocol")]
    Protocol,
    #[error("invalid connection string: {0}")]
    InvalidConnectionString(String),
}

#[derive(Clone, Debug)]
pub struct DataStore {
    root_directory: Arc<PathBuf>,
}

impl DataStore {
    pub fn open(root_directory: impl Into<PathBuf>) -> Result<Self, DataStoreError> {
        let root_directory = root_directory.into();
        if !root_directory.is_dir() {
            return Err(DataStoreError::InvalidRoot(root_directory));
        }

        Ok(Self {
            root_directory: Arc::new(root_directory),
        })
    }

    pub async fn store(&self, data: &[u8]) -> Result<String, DataStoreError> {
        let id = Uuid::new_v4().hyphenated().to_string();
        let object_path = object_path(&self.root_directory, &id);
        let object_dir = object_path
            .parent()
            .expect("object store path always has a parent");

        fs::create_dir_all(object_dir).await?;

        if let Err(error) = fs::write(&object_path, data).await {
            let _ = fs::remove_file(&object_path).await;
            return Err(DataStoreError::Io(error));
        }

        Ok(id)
    }

    pub async fn load(&self, id: &str) -> Result<Option<Vec<u8>>, DataStoreError> {
        if !validate_object_id(id) {
            return Ok(None);
        }

        let path = object_path(&self.root_directory, id);
        match fs::read(path).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(DataStoreError::Io(error)),
        }
    }

    pub async fn delete(&self, id: &str) -> Result<bool, DataStoreError> {
        if !validate_object_id(id) {
            return Ok(false);
        }

        let path = object_path(&self.root_directory, id);
        match fs::remove_file(path).await {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(DataStoreError::Io(error)),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ObjectStoreServer {
    data_store: DataStore,
}

impl ObjectStoreServer {
    pub fn new(data_store: DataStore) -> Self {
        Self { data_store }
    }

    pub async fn store(&self, data: &[u8]) -> Result<String, ObjectStoreError> {
        let id = self.data_store.store(data).await?;
        info!(object_id = %id, "stored object");
        Ok(id)
    }

    pub async fn load(&self, id: &str) -> Result<Option<Vec<u8>>, ObjectStoreError> {
        info!(object_id = %id, "loading object");
        self.data_store.load(id).await.map_err(Into::into)
    }

    pub async fn delete(&self, id: &str) -> Result<bool, ObjectStoreError> {
        info!(object_id = %id, "deleting object");
        self.data_store.delete(id).await.map_err(Into::into)
    }
}

pub async fn serve<S>(
    listener: TcpListener,
    server: ObjectStoreServer,
    shutdown: S,
) -> Result<(), ObjectStoreError>
where
    S: Future<Output = ()> + Send,
{
    let server = Arc::new(server);
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            accept_result = listener.accept() => {
                let (stream, peer) = accept_result?;
                let server = Arc::clone(&server);
                tokio::spawn(async move {
                    if let Err(error) = handle_connection(stream, peer, server).await {
                        warn!(peer = %peer, %error, "object store connection ended with an error");
                    }
                });
            }
            _ = &mut shutdown => break,
        }
    }

    Ok(())
}

async fn handle_connection(
    stream: TcpStream,
    peer: SocketAddr,
    server: Arc<ObjectStoreServer>,
) -> Result<(), ObjectStoreError> {
    let mut stream = BufStream::new(stream);
    let mut line = String::new();

    loop {
        line.clear();
        let read = stream.read_line(&mut line).await?;
        if read == 0 {
            info!(peer = %peer, "object store connection closed");
            return Ok(());
        }

        let command = line.trim_end_matches('\n').trim_end_matches('\r');
        if !handle_command(&mut stream, &server, command).await? {
            warn!(peer = %peer, command, "closing connection because of malformed command");
            return Err(ObjectStoreError::Protocol);
        }
    }
}

async fn handle_command(
    stream: &mut BufStream<TcpStream>,
    server: &ObjectStoreServer,
    command: &str,
) -> Result<bool, ObjectStoreError> {
    let (verb, tail) = match command.split_once(' ') {
        Some(parts) => parts,
        None => return Ok(false),
    };

    match verb {
        "STORE" => {
            let length: usize = tail.parse().map_err(|_| ObjectStoreError::Protocol)?;
            let mut data = vec![0; length];
            stream.read_exact(&mut data).await?;

            match server.store(&data).await {
                Ok(id) => write_line(stream, &format!("OK {id}")).await?,
                Err(error) => {
                    error!(%error, "failed to store object");
                    write_line(stream, "ERR").await?;
                }
            }

            Ok(true)
        }
        "GET" => {
            if !validate_object_id(tail) {
                return Ok(false);
            }

            match server.load(tail).await {
                Ok(Some(data)) => {
                    write_line(stream, &format!("OK {}", data.len())).await?;
                    stream.write_all(&data).await?;
                    stream.flush().await?;
                }
                Ok(None) => write_line(stream, "ERR").await?,
                Err(error) => {
                    error!(object_id = tail, %error, "failed to load object");
                    write_line(stream, "ERR").await?;
                }
            }

            Ok(true)
        }
        "DEL" => {
            if !validate_object_id(tail) {
                return Ok(false);
            }

            match server.delete(tail).await {
                Ok(_) => write_line(stream, "OK").await?,
                Err(error) => {
                    error!(object_id = tail, %error, "failed to delete object");
                    write_line(stream, "ERR").await?;
                }
            }

            Ok(true)
        }
        _ => Ok(false),
    }
}

async fn write_line(stream: &mut BufStream<TcpStream>, line: &str) -> Result<(), ObjectStoreError> {
    stream.write_all(line.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    stream.flush().await?;
    Ok(())
}

#[derive(Clone, Debug)]
pub struct ObjectStoreClient {
    stream: Arc<Mutex<BufStream<TcpStream>>>,
}

impl ObjectStoreClient {
    pub async fn connect(connection_string: &str) -> Result<Self, ObjectStoreError> {
        let address = parse_connection_string(connection_string, 5396)?;
        let stream = TcpStream::connect(address).await?;
        Ok(Self {
            stream: Arc::new(Mutex::new(BufStream::new(stream))),
        })
    }

    pub async fn store(&self, data: &[u8]) -> Result<Option<String>, ObjectStoreError> {
        let mut stream = self.stream.lock().await;
        write_line(&mut stream, &format!("STORE {}", data.len())).await?;
        stream.write_all(data).await?;
        stream.flush().await?;

        match read_line(&mut stream).await?.as_deref() {
            Some(response) => parse_store_response(response),
            None => Ok(None),
        }
    }

    pub async fn get(&self, id: &str) -> Result<Option<Vec<u8>>, ObjectStoreError> {
        if !validate_object_id(id) {
            return Err(ObjectStoreError::Protocol);
        }

        let mut stream = self.stream.lock().await;
        write_line(&mut stream, &format!("GET {id}")).await?;

        match read_line(&mut stream).await?.as_deref() {
            Some(response) => parse_get_response(&mut stream, response).await,
            None => Ok(None),
        }
    }

    pub async fn delete(&self, id: &str) -> Result<bool, ObjectStoreError> {
        if !validate_object_id(id) {
            return Err(ObjectStoreError::Protocol);
        }

        let mut stream = self.stream.lock().await;
        write_line(&mut stream, &format!("DEL {id}")).await?;

        match read_line(&mut stream).await?.as_deref() {
            Some("OK") => Ok(true),
            Some("ERR") => Ok(false),
            Some(_) => Err(ObjectStoreError::Protocol),
            None => Ok(false),
        }
    }
}

fn parse_store_response(response: &str) -> Result<Option<String>, ObjectStoreError> {
    if response == "ERR" {
        return Ok(None);
    }

    let (status, id) = response
        .split_once(' ')
        .ok_or(ObjectStoreError::Protocol)?;
    if status != "OK" || !validate_object_id(id) {
        return Err(ObjectStoreError::Protocol);
    }

    Ok(Some(id.to_owned()))
}

async fn parse_get_response(
    stream: &mut BufStream<TcpStream>,
    response: &str,
) -> Result<Option<Vec<u8>>, ObjectStoreError> {
    if response == "ERR" {
        return Ok(None);
    }

    let (status, length) = response
        .split_once(' ')
        .ok_or(ObjectStoreError::Protocol)?;
    if status != "OK" {
        return Err(ObjectStoreError::Protocol);
    }

    let length: usize = length.parse().map_err(|_| ObjectStoreError::Protocol)?;
    let mut data = vec![0; length];
    stream.read_exact(&mut data).await?;
    Ok(Some(data))
}

async fn read_line(stream: &mut BufStream<TcpStream>) -> Result<Option<String>, ObjectStoreError> {
    let mut response = String::new();
    let read = stream.read_line(&mut response).await?;
    if read == 0 {
        return Ok(None);
    }

    Ok(Some(
        response
            .trim_end_matches('\n')
            .trim_end_matches('\r')
            .to_owned(),
    ))
}

fn parse_connection_string(
    connection_string: &str,
    default_port: u16,
) -> Result<std::net::SocketAddr, ObjectStoreError> {
    let (host, port) = match connection_string.split_once(':') {
        Some((host, port)) => {
            let port = port.parse::<u16>().map_err(|_| {
                ObjectStoreError::InvalidConnectionString(connection_string.to_owned())
            })?;
            (host, port)
        }
        None => (connection_string, default_port),
    };

    (host, port)
        .to_socket_addrs()
        .map_err(|_| ObjectStoreError::InvalidConnectionString(connection_string.to_owned()))?
        .next()
        .ok_or_else(|| ObjectStoreError::InvalidConnectionString(connection_string.to_owned()))
}

pub fn validate_object_id(id: &str) -> bool {
    Uuid::parse_str(id)
        .map(|uuid| uuid.hyphenated().to_string() == id)
        .unwrap_or(false)
}

fn object_path(root_directory: &Path, id: &str) -> PathBuf {
    root_directory.join(&id[..2]).join(id)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tempfile::tempdir;
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;

    use super::{serve, DataStore, ObjectStoreClient, ObjectStoreServer};

    #[tokio::test]
    async fn objectstore_round_trip() {
        let temp_dir = tempdir().expect("tempdir should exist");
        let data_store = DataStore::open(temp_dir.path()).expect("datastore should open");
        let server = ObjectStoreServer::new(data_store);
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let address = listener
            .local_addr()
            .expect("listener should have a local address");

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let server_task = tokio::spawn(async move {
            serve(listener, server, async move {
                let _ = shutdown_rx.await;
            })
            .await
        });

        let client = ObjectStoreClient::connect(&address.to_string())
            .await
            .expect("client should connect");
        let payload = b"vienna-rust".to_vec();

        let object_id = client
            .store(&payload)
            .await
            .expect("store should succeed")
            .expect("store should return an object id");

        let loaded = client
            .get(&object_id)
            .await
            .expect("get should succeed")
            .expect("stored object should exist");
        assert_eq!(loaded, payload);

        assert!(client
            .delete(&object_id)
            .await
            .expect("delete should succeed"));

        let missing = client.get(&object_id).await.expect("get should succeed");
        assert!(missing.is_none());

        shutdown_tx.send(()).expect("shutdown should be sent");
        tokio::time::timeout(Duration::from_secs(5), server_task)
            .await
            .expect("server should stop")
            .expect("server task should join")
            .expect("server should exit cleanly");
    }
}
