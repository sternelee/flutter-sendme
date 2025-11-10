use crate::{ProgressInfo, ProgressOperation, ProgressSender, ReceiveResult, SendResult, SENDME_STATE};
use anyhow::Context;
use data_encoding::HEXLOWER;
use iroh::{Endpoint, RelayMode, SecretKey};
use iroh_blobs::{
    api::{
        blobs::{AddPathOptions, ExportMode, ExportOptions, ImportMode},
        remote::GetProgressItem,
        Store, TempTag,
    },
    format::collection::Collection,
    get::request::get_hash_seq_and_sizes,
    provider::events::{EventMask, EventSender},
    store::fs::FsStore,
    ticket::BlobTicket,
    BlobFormat, BlobsProtocol,
};
use n0_future::{BufferedStreamExt, StreamExt};
use rand::Rng;
use std::str::FromStr;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use walkdir::WalkDir;

#[flutter_rust_bridge::frb(sync)]
pub fn init_logging() {
    tracing_subscriber::fmt::init();
}

#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
    flutter_rust_bridge::setup_default_user_utils();
    init_logging();
}

fn get_or_create_secret() -> anyhow::Result<SecretKey> {
    match std::env::var("IROH_SECRET") {
        Ok(secret) => SecretKey::from_str(&secret).context("invalid secret"),
        Err(_) => {
            let key = SecretKey::generate(&mut rand::rng());
            Ok(key)
        }
    }
}

async fn import_with_progress(
    path: PathBuf,
    db: &Store,
    progress_sender: ProgressSender,
) -> anyhow::Result<(TempTag, u64, Collection)> {
    let parallelism = num_cpus::get();
    let path = path.canonicalize()?;
    anyhow::ensure!(path.exists(), "path {} does not exist", path.display());
    let root = path.parent().context("get parent")?;

    let files = WalkDir::new(path.clone()).into_iter();
    let data_sources: Vec<(String, PathBuf)> = files
        .map(|entry| {
            let entry = entry?;
            if !entry.file_type().is_file() {
                return Ok(None);
            }
            let path = entry.into_path();
            let relative = path.strip_prefix(root)?;
            let name = relative.to_string_lossy().to_string();
            anyhow::Ok(Some((name, path)))
        })
        .filter_map(Result::transpose)
        .collect::<anyhow::Result<Vec<_>>>()?;

    let total_files = data_sources.len() as u64;
    let mut processed_files = 0u64;

    if let Some(sender) = progress_sender.lock().unwrap().as_ref() {
        let _ = sender.send(ProgressInfo {
            operation: ProgressOperation::Import,
            current: 0,
            total: total_files,
            message: format!("Importing {} files", total_files),
        });
    }

    let mut names_and_tags: Vec<(String, TempTag, u64)> = n0_future::stream::iter(data_sources)
        .map(|(name, path)| {
            let db = db.clone();
            let progress_sender = progress_sender.clone();
            async move {
                let import = db.add_path_with_opts(AddPathOptions {
                    path,
                    mode: ImportMode::TryReference,
                    format: BlobFormat::Raw,
                });
                let mut stream = import.stream().await;
                let mut item_size = 0;
                let temp_tag = loop {
                    let item = stream
                        .next()
                        .await
                        .context("import stream ended without a tag")?;
                    match item {
                        iroh_blobs::api::blobs::AddProgressItem::Size(size) => {
                            item_size = size;
                        }
                        iroh_blobs::api::blobs::AddProgressItem::Done(tt) => {
                            break tt;
                        }
                        iroh_blobs::api::blobs::AddProgressItem::Error(cause) => {
                            anyhow::bail!("error importing {}: {}", name, cause);
                        }
                        _ => {}
                    }
                };
                anyhow::Ok((name, temp_tag, item_size))
            }
        })
        .buffered_unordered(parallelism)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<anyhow::Result<Vec<_>>>()?;

    processed_files = names_and_tags.len() as u64;

    if let Some(sender) = progress_sender.lock().unwrap().as_ref() {
        let _ = sender.send(ProgressInfo {
            operation: ProgressOperation::Import,
            current: processed_files,
            total: total_files,
            message: format!("Processed {} files", processed_files),
        });
    }

    names_and_tags
        .sort_by(|(a, _, _): &(String, TempTag, u64), (b, _, _): &(String, TempTag, u64)| a.cmp(b));
    let size = names_and_tags.iter().map(|(_, _, size)| *size).sum::<u64>();
    let (collection, tags) = names_and_tags
        .into_iter()
        .map(|(name, tag, _): (String, TempTag, u64)| ((name, tag.hash()), tag))
        .unzip::<_, _, Collection, Vec<_>>();
    let temp_tag = collection.clone().store(db).await?;
    drop(tags);

    Ok((temp_tag, size, collection))
}

async fn export_with_progress(
    db: &Store,
    collection: Collection,
    progress_sender: ProgressSender,
) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let total_files = collection.len() as u64;
    let mut processed_files = 0u64;

    for (i, (name, hash)) in collection.iter().enumerate() {
        processed_files = i as u64;

        if let Some(sender) = progress_sender.lock().unwrap().as_ref() {
            let _ = sender.send(ProgressInfo {
                operation: ProgressOperation::Export,
                current: processed_files,
                total: total_files,
                message: format!("Exporting {}", name),
            });
        }

        let target = root.join(name);
        if target.exists() {
            anyhow::bail!("target {} already exists", target.display());
        }

        let mut stream = db
            .export_with_opts(ExportOptions {
                hash: *hash,
                target,
                mode: ExportMode::Copy,
            })
            .stream()
            .await;

        while let Some(item) = stream.next().await {
            match item {
                iroh_blobs::api::blobs::ExportProgressItem::Done => {
                    // File exported successfully
                }
                iroh_blobs::api::blobs::ExportProgressItem::Error(cause) => {
                    anyhow::bail!("error exporting {}: {}", name, cause);
                }
                _ => {}
            }
        }
    }

    if let Some(sender) = progress_sender.lock().unwrap().as_ref() {
        let _ = sender.send(ProgressInfo {
            operation: ProgressOperation::Export,
            current: total_files,
            total: total_files,
            message: "Export completed".to_string(),
        });
    }

    Ok(())
}

#[flutter_rust_bridge::frb]
pub async fn send_file(path: String) -> anyhow::Result<SendResult> {
    println!("send_file called with path: {}", path);
    let path = PathBuf::from(path);
    let secret_key = get_or_create_secret()?;
    println!("Secret key created successfully");

    let suffix = rand::rng().random::<[u8; 16]>();
    let cwd = std::env::current_dir()?;
    let blobs_data_dir = cwd.join(format!(".sendme-send-{}", HEXLOWER.encode(&suffix)));

    tokio::fs::create_dir_all(&blobs_data_dir).await?;
    let store = FsStore::load(&blobs_data_dir).await?;
    println!("Store created successfully");

    let (temp_tag, size, collection) = import_with_progress(
        path.clone(),
        &store,
        Arc::new(Mutex::new(None)), // Keep disabled for now
    )
    .await?;
    let hash = temp_tag.hash();
    println!("File imported successfully, hash: {}", hash.to_hex());

    println!("Creating endpoint...");
    let endpoint = Endpoint::builder()
        .alpns(vec![iroh_blobs::protocol::ALPN.to_vec()])
        .secret_key(secret_key)
        .bind()
        .await?;
    println!("Endpoint created successfully");

    let blobs = BlobsProtocol::new(
        &store,
        Some(EventSender::new(mpsc::channel(32).0, EventMask::default())),
    );

    println!("Creating router...");
    let router = iroh::protocol::Router::builder(endpoint)
        .accept(iroh_blobs::ALPN, blobs.clone())
        .spawn();
    println!("Router created successfully");

    // Skip the endpoint online check and add a delay instead
    println!("Skipping endpoint online check, using delay instead...");
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("Continuing after delay...");

    let addr = router.endpoint().addr();
    println!("Got endpoint address: {:?}", addr);
    let ticket = BlobTicket::new(addr, hash, BlobFormat::HashSeq);
    let file_count = collection.len() as u64;

    let ticket_string = ticket.to_string();
    let result = SendResult {
        ticket: ticket_string.clone(),
        hash: hash.to_hex().to_string(),
        size,
        file_count,
    };

    // Store the sender in global state to keep it alive
    // We use a tuple to keep both router and temp_tag alive
    let sender_tuple = (router, temp_tag);
    SENDME_STATE.add_sender(ticket_string.clone(), Box::new(sender_tuple));

    Ok(result)
}

#[flutter_rust_bridge::frb]
pub async fn receive_file(ticket: String) -> anyhow::Result<ReceiveResult> {
    let ticket = BlobTicket::from_str(&ticket)?;
    let secret_key = get_or_create_secret()?;

    let progress_sender: ProgressSender = Arc::new(Mutex::new(None)); // Keep disabled for now

    let endpoint = Endpoint::builder()
        .alpns(vec![])
        .secret_key(secret_key)
        .bind()
        .await?;

    let dir_name = format!(".sendme-recv-{}", ticket.hash().to_hex());
    let iroh_data_dir = std::env::current_dir()?.join(dir_name);
    let store = FsStore::load(&iroh_data_dir).await?;

    let hash_and_format = ticket.hash_and_format();
    let local = store.remote().local(hash_and_format).await?;
    let t0 = Instant::now();

    if !local.is_complete() {
        let connection = endpoint
            .connect(ticket.addr().clone(), iroh_blobs::protocol::ALPN)
            .await?;

        let (_hash_seq, sizes) =
            get_hash_seq_and_sizes(&connection, &hash_and_format.hash, 1024 * 1024 * 32, None)
                .await?;

        let _total_size = sizes.iter().copied().sum::<u64>();
        let _payload_size = sizes.iter().skip(2).copied().sum::<u64>();
        let _total_files = (sizes.len().saturating_sub(1)) as u64;

        let get = store.remote().execute_get(connection, local.missing());
        let mut stream = get.stream();

        while let Some(item) = stream.next().await {
            match item {
                GetProgressItem::Progress(_offset) => {
                    // Progress handling disabled for now
                }
                GetProgressItem::Done(_) => break,
                GetProgressItem::Error(cause) => {
                    anyhow::bail!("Download error: {:?}", cause);
                }
                _ => {}
            }
        }
    }

    let collection = Collection::load(hash_and_format.hash, store.as_ref()).await?;
    let file_count = collection.len() as u64;
    export_with_progress(&store, collection, progress_sender).await?;

    let duration = t0.elapsed();
    tokio::fs::remove_dir_all(iroh_data_dir).await?;

    // Get the local data for the received collection
    let local_data = store.remote().local(hash_and_format).await?;
    let result = ReceiveResult {
        file_count,
        size: local_data.local_bytes(),
        duration_ms: duration.as_millis() as u64,
    };

    Ok(result)
}

// Add bytesize dependency for better formatting
#[flutter_rust_bridge::frb]
pub fn format_bytes(size: u64) -> String {
    bytesize::ByteSize::b(size).to_string()
}

