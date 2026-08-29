use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::config::RequestChecksumCalculation;
use aws_sdk_s3::primitives::ByteStream;
use clap::Parser;

#[derive(Parser)]
#[command(
    version,
    about = "Upload rendered map images to OCI Object Storage via the S3 compatibility API",
    after_help = "Credentials are read from AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY\n(an OCI Customer Secret Key)."
)]
struct Args {
    #[arg(
        short,
        long,
        value_name = "DIR",
        help = "Directory of PNGs to upload; object names are the file names"
    )]
    input: PathBuf,

    #[arg(long, value_name = "REGION", help = "OCI region, e.g. us-ashburn-1")]
    region: String,

    #[arg(long, default_value = "map-images", help = "Target bucket")]
    bucket: String,

    #[arg(
        long,
        default_value = "ax9n3v6n1qzw",
        help = "OCI Object Storage namespace"
    )]
    namespace: String,

    #[arg(
        short,
        long,
        value_name = "N",
        default_value_t = 8,
        help = "Concurrent uploads"
    )]
    jobs: usize,

    #[arg(long, help = "List what would be uploaded without uploading")]
    dry_run: bool,
}

fn find_pngs(dir: &PathBuf) -> Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "png"))
        .collect();
    files.sort();
    Ok(files)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let files = find_pngs(&args.input)?;
    anyhow::ensure!(
        !files.is_empty(),
        "no .png files in {}",
        args.input.display()
    );

    if args.dry_run {
        for file in &files {
            println!("{}", file.file_name().unwrap().to_string_lossy());
        }
        println!("{} files (dry run)", files.len());
        return Ok(());
    }

    let endpoint = format!(
        "https://{}.compat.objectstorage.{}.oraclecloud.com",
        args.namespace, args.region
    );
    let base = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new(args.region.clone()))
        .endpoint_url(endpoint)
        .load()
        .await;
    let config = aws_sdk_s3::config::Builder::from(&base)
        .force_path_style(true)
        .request_checksum_calculation(RequestChecksumCalculation::WhenRequired)
        .build();
    let client = aws_sdk_s3::Client::from_conf(config);

    let semaphore = Arc::new(tokio::sync::Semaphore::new(args.jobs.max(1)));
    let mut tasks = tokio::task::JoinSet::new();
    for file in files {
        let permit = semaphore.clone().acquire_owned().await?;
        let client = client.clone();
        let bucket = args.bucket.clone();
        tasks.spawn(async move {
            let name = file.file_name().unwrap().to_string_lossy().into_owned();
            let result = upload(&client, &bucket, &name, &file).await;
            drop(permit);
            (name, result)
        });
    }

    let mut uploaded = 0usize;
    let mut failed = 0usize;
    while let Some(joined) = tasks.join_next().await {
        let (name, result) = joined.context("upload task panicked")?;
        match result {
            Ok(()) => {
                uploaded += 1;
                println!("{name}");
            }
            Err(e) => {
                failed += 1;
                eprintln!("FAIL {name}: {e:#}");
            }
        }
    }

    println!("{uploaded} uploaded, {failed} failed");
    anyhow::ensure!(failed == 0, "{failed} uploads failed");
    Ok(())
}

async fn upload(
    client: &aws_sdk_s3::Client,
    bucket: &str,
    name: &str,
    file: &PathBuf,
) -> Result<()> {
    let body = ByteStream::from_path(file)
        .await
        .with_context(|| format!("reading {}", file.display()))?;
    client
        .put_object()
        .bucket(bucket)
        .key(name)
        .content_type("image/png")
        .body(body)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!(e.into_service_error()))?;
    Ok(())
}
