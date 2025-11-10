use crate::{ProgressInfo, ProgressOperation, ProgressSender, ReceiveResult, SendResult, SENDME_STATE};
use anyhow::Context;
use data_encoding::HEXLOWER;
use iroh::{Endpoint, SecretKey};
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
            message: format!("正在导入 {} 个文件", total_files),
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
            message: format!("已处理 {} 个文件", processed_files),
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
                message: format!("正在导出 {}", name),
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
            message: "导出完成".to_string(),
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

    // Enable progress tracking with a real sender
    let (progress_stream, progress_sender) = crate::ProgressStream::new();

    // Store the progress stream globally for access
    let stream_clone = progress_stream;
    SENDME_STATE.add_sender("progress_stream".to_string(), Box::new(stream_clone));

    let (temp_tag, size, collection) = import_with_progress(
        path.clone(),
        &store,
        progress_sender.clone(),
    )
    .await?;
    let hash = temp_tag.hash();
    println!("File imported successfully, hash: {}", hash.to_hex());

    // Send completion progress
    if let Some(sender) = progress_sender.lock().unwrap().as_ref() {
        let _ = sender.send(crate::ProgressInfo {
            operation: crate::ProgressOperation::Import,
            current: 1,
            total: 1,
            message: "文件导入完成，正在等待接收方连接...".to_string(),
        });
    }

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
    println!("Creating ticket for file transfer...");
    let ticket = BlobTicket::new(addr, hash, BlobFormat::HashSeq);
    println!("Created ticket: {}", ticket.to_string());
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
    let sender_tuple = (router, temp_tag, progress_sender);
    SENDME_STATE.add_sender(ticket_string.clone(), Box::new(sender_tuple));

    println!("Sender setup complete. Keeping connection alive for ticket: {}", ticket_string);
    println!("Waiting for receiver to connect...");

    Ok(result)
}

#[flutter_rust_bridge::frb]
pub async fn receive_file(ticket: String) -> anyhow::Result<ReceiveResult> {
    let ticket = BlobTicket::from_str(&ticket)?;
    let secret_key = get_or_create_secret()?;

    // Enable progress tracking
    let (progress_stream, progress_sender) = crate::ProgressStream::new();

    // Store the progress stream globally for access
    SENDME_STATE.add_sender("receive_progress".to_string(), Box::new(progress_stream));

    // Send initial progress
    if let Some(sender) = progress_sender.lock().unwrap().as_ref() {
        let _ = sender.send(crate::ProgressInfo {
            operation: crate::ProgressOperation::Connect,
            current: 0,
            total: 1,
            message: "正在解析 ticket...".to_string(),
        });
    }

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

    // Send connection progress
    if let Some(sender) = progress_sender.lock().unwrap().as_ref() {
        let _ = sender.send(crate::ProgressInfo {
            operation: crate::ProgressOperation::Connect,
            current: 1,
            total: 3,
            message: "正在连接到发送方...".to_string(),
        });
    }

    if !local.is_complete() {
        // Add timeout for connection attempt
        println!("Attempting to connect to sender at: {:?}", ticket.addr());

        let connection = tokio::time::timeout(
            Duration::from_secs(30), // 30 second timeout
            endpoint.connect(ticket.addr().clone(), iroh_blobs::protocol::ALPN)
        ).await
        .map_err(|_| anyhow::anyhow!("连接超时：无法在30秒内连接到发送方。请确保：\n1. 发送方仍在运行\n2. 网络连接正常\n3. Ticket 正确且未过期\n4. 防火墙没有阻止连接"))??;

        // Send connection established progress
        if let Some(sender) = progress_sender.lock().unwrap().as_ref() {
            let _ = sender.send(crate::ProgressInfo {
                operation: crate::ProgressOperation::Connect,
                current: 2,
                total: 3,
                message: "已连接，正在获取文件信息...".to_string(),
            });
        }

        let (_hash_seq, sizes) =
            get_hash_seq_and_sizes(&connection, &hash_and_format.hash, 1024 * 1024 * 32, None)
                .await?;

        let total_size = sizes.iter().copied().sum::<u64>();
        let payload_size = sizes.iter().skip(2).copied().sum::<u64>();
        let total_files = (sizes.len().saturating_sub(1)) as u64;

        // Send download start progress
        if let Some(sender) = progress_sender.lock().unwrap().as_ref() {
            let _ = sender.send(crate::ProgressInfo {
                operation: crate::ProgressOperation::Download,
                current: 0,
                total: total_size,
                message: format!("开始下载 {} 个文件，总大小: {}", total_files, format_bytes(total_size)),
            });
        }

        let get = store.remote().execute_get(connection, local.missing());
        let mut stream = get.stream();
        let mut last_progress = 0u64;

        while let Some(item) = stream.next().await {
            match item {
                GetProgressItem::Progress(offset) => {
                    // Send real download progress
                    let progress = offset as u64;
                    if progress - last_progress >= total_size / 100 || progress == total_size { // Update every 1%
                        if let Some(sender) = progress_sender.lock().unwrap().as_ref() {
                            let _ = sender.send(crate::ProgressInfo {
                                operation: crate::ProgressOperation::Download,
                                current: progress,
                                total: total_size,
                                message: format!("正在下载... {}/{} ({:.1}%)",
                                    format_bytes(progress),
                                    format_bytes(total_size),
                                    (progress as f64 / total_size as f64) * 100.0),
                            });
                        }
                        last_progress = progress;
                    }
                }
                GetProgressItem::Done(_) => {
                    // Send download completion progress
                    if let Some(sender) = progress_sender.lock().unwrap().as_ref() {
                        let _ = sender.send(crate::ProgressInfo {
                            operation: crate::ProgressOperation::Download,
                            current: total_size,
                            total: total_size,
                            message: "下载完成，正在导出文件...".to_string(),
                        });
                    }
                    break;
                }
                GetProgressItem::Error(cause) => {
                    anyhow::bail!("Download error: {:?}", cause);
                }
                _ => {}
            }
        }
    }

    let collection = Collection::load(hash_and_format.hash, store.as_ref()).await?;
    let file_count = collection.len() as u64;
    export_with_progress(&store, collection, progress_sender.clone()).await?;

    // Send final completion progress
    if let Some(sender) = progress_sender.lock().unwrap().as_ref() {
        let _ = sender.send(crate::ProgressInfo {
            operation: crate::ProgressOperation::Export,
            current: 1,
            total: 1,
            message: "文件接收完成！".to_string(),
        });
    }

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

// Function to validate a ticket format
#[flutter_rust_bridge::frb]
pub fn validate_ticket(ticket: String) -> anyhow::Result<String> {
    match BlobTicket::from_str(&ticket) {
        Ok(parsed_ticket) => {
            Ok(format!("Ticket 有效\n地址: {:?}\n哈希: {}\n格式: {:?}",
                      parsed_ticket.addr(),
                      parsed_ticket.hash().to_hex(),
                      parsed_ticket.hash_and_format().format))
        }
        Err(e) => anyhow::bail!("Ticket 格式无效: {}", e)
    }
}


