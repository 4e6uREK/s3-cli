use clap::Parser;
use tokio::fs;
use tokio::fs::File;
use std::error::Error;
use std::io::Read;
use std::path::Path;
use serde::Deserialize;
use tokio::io::AsyncReadExt;
use rusoto_core::{HttpClient, Region};
use rusoto_credential::StaticProvider;
use rusoto_s3::{ListObjectsV2Request, PutObjectRequest, GetObjectRequest, S3Client, S3};
use home::home_dir;
use tar::{Builder, Archive};

#[derive(Debug, Default, Deserialize)]
struct Config {
    domain: String,
    region: String,
    access_key: String,
    secret_key: String,
    bucket: String,
}

#[derive(Debug, Parser)]
#[command(about = "4e6uREK's S3 CLI", long_about = None)]
struct Cli {
    #[arg(short, long)]
    config: Option<String>,

    #[arg(short, long, action = clap::ArgAction::Append)]
    send: Option<Vec<String>>,

    #[arg(short, long, action = clap::ArgAction::Append)]
    recv: Option<Vec<String>>,

    #[arg(short, long, default_value = "false")]
    dump: bool,

    #[arg(short, long)]
    populate: Option<String>,

    #[arg(short, long, default_value = "false")]
    list: bool,
}

async fn send_file(cfg: &Config, file: &str) -> Result<String, Box<dyn Error>> {
    let region = Region::Custom {
        name: cfg.region.clone(),
        endpoint: cfg.domain.clone(),
    };

    let provider = StaticProvider::new_minimal(cfg.access_key.clone(), cfg.secret_key.clone());
    let http_client = HttpClient::new()?;

    let s3_client = S3Client::new_with(http_client, provider, region);

    let data = fs::read(file).await?;
    let filepath = Path::new(file).file_name().unwrap().to_str().unwrap();

    let req = PutObjectRequest {
        bucket: "proxima-torrents".to_string(),
        key: filepath.to_string(),
        body: Some(data.into()),
        ..Default::default()
    };

    match s3_client.put_object(req).await {
        Ok(_) => { 
            Ok(filepath.to_string())
        },
        Err(e) => Err(e)?,
    }
}

async fn recv_file(cfg: &Config, file: &str) -> Result<String, Box<dyn Error>> {
    let region = Region::Custom {
        name: cfg.region.clone(),
        endpoint: cfg.domain.clone(),
    };

    let provider = StaticProvider::new_minimal(cfg.access_key.clone(), cfg.secret_key.clone());
    let http_client = HttpClient::new()?;

    let s3_client = S3Client::new_with(http_client, provider, region);

    let req = GetObjectRequest {
        bucket: cfg.bucket.clone(),
        key: file.to_string(),
        ..Default::default()
    };

    match s3_client.get_object(req).await {
        Ok(output) => {
            let body = output.body.unwrap();
            let mut body_reader = body.into_async_read();
            let mut file_content = Vec::new();

            if let Err(e) = body_reader.read_to_end(&mut file_content).await {
                eprintln!("Error reading file from S3: {:?}", e);
            } else {
                if let Err(e) = fs::write(&file, &file_content).await {
                    eprintln!("Error saving file locally: {:?}",  e);
                }
            }
        },
        Err(e) => Err(e)?,
    }

    Ok(format!("{}/{}/{} -> {}", &cfg.domain, &cfg.bucket, &file, &file))
}

async fn list_objects(cfg: &Config) -> Result<Vec<String>, Box<dyn Error>> {
    let region = Region::Custom {
        name: cfg.region.clone(),
        endpoint: cfg.domain.clone(),
    };

    let provider = StaticProvider::new_minimal(cfg.access_key.clone(), cfg.secret_key.clone());
    let http_client = HttpClient::new()?;

    let s3_client = S3Client::new_with(http_client, provider, region);

    let mut out: Vec<String> = Vec::new();

    let list_req = ListObjectsV2Request {
        bucket: cfg.bucket.clone(),
        ..Default::default()
    };

    match s3_client.list_objects_v2(list_req).await {
        Ok(output) => {
            if let Some(contents) = output.contents {
                for object in contents {
                    out.push(object.key.unwrap());
                }
            }
        },
        Err(e) => Err(e)?,
    }

    Ok(out)
}

async fn dump(cfg: &Config) -> Result<String, Box<dyn Error>> {
    let files = list_objects(&cfg).await?;

    let region = Region::Custom {
        name: cfg.region.clone(),
        endpoint: cfg.domain.clone(),
    };

    let provider = StaticProvider::new_minimal(cfg.access_key.clone(), cfg.secret_key.clone());
    let http_client = HttpClient::new()?;

    let s3_client = S3Client::new_with(http_client, provider, region);

    let archive_name = format!("{}_dump.tar", &cfg.bucket);
    let archive = File::create(&archive_name).await?;
    let mut builder = Builder::new(archive.into_std().await);

    for file in files {
        let req = GetObjectRequest {
            bucket: cfg.bucket.clone(),
            key: file.to_string(),
            ..Default::default()
        };

        match s3_client.get_object(req).await {
            Ok(output) => {
                let body = output.body.unwrap();
                let mut body_reader = body.into_async_read();
                let mut file_content = Vec::new();

                if let Err(e) = body_reader.read_to_end(&mut file_content).await {
                    eprintln!("{}/{}/{} -> Error: {:?}", &cfg.domain, &cfg.bucket, &file, e);
                } else {
                    let mut header = tar::Header::new_gnu();
                    header.set_size(file_content.len() as u64);
                    header.set_cksum();

                    builder.append_data(&mut header, &file, &file_content[..])?;
                    println!("{}/{}/{} -> OK", &cfg.domain, &cfg.bucket, &file);
                }
            },
            Err(e) => Err(e)?
        }
    }

    println!();
    Ok(format!("{}/{} -> {}", &cfg.domain, &cfg.bucket, &archive_name))
}

async fn populate(cfg: &Config, archive_path: &String) -> Result<String, Box<dyn Error>> {
    let mut raw_archive = File::open(&archive_path).await?;

    let mut buffer = Vec::new();
    raw_archive.read_to_end(&mut buffer).await?;

    let mut archive = Archive::new(&*buffer);

    let region = Region::Custom {
        name: cfg.region.clone(),
        endpoint: cfg.domain.clone(),
    };

    let provider = StaticProvider::new_minimal(cfg.access_key.clone(), cfg.secret_key.clone());
    let http_client = HttpClient::new()?;

    let s3_client = S3Client::new_with(http_client, provider, region);

    for file in archive.entries()? {
        let mut file = file?;
        let mut data: Vec<u8> = Vec::new();
        file.read_to_end(&mut data)?;

        let filename = file.header().path().unwrap().to_str().unwrap().to_owned();

        let req = PutObjectRequest {
            bucket: cfg.bucket.clone(),
            key: filename.clone(),
            body: Some(data.into()),
            ..Default::default()
        };

        match s3_client.put_object(req).await {
            Ok(_) => println!("{}/{}/{} <- {}", &cfg.domain, &cfg.bucket, &filename, &filename),
            Err(e) => Err(e)?,
        }
    }

    println!();
    Ok(format!("{}/{} <- {}", &cfg.domain, &cfg.bucket, &archive_path))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut config: Config = Default::default();

    let args = Cli::parse();

    if let Some(cfg) = args.config {
        let ini_content = match fs::read_to_string(&cfg).await {
            Ok(data) => data,
            Err(_) => panic!("Failed to read provided configuration file"),
        };
        config = serde_ini::from_str(&ini_content)?;
    } else {
        let path = match home_dir() {
            Some(data) => data.to_str().unwrap().to_owned(),
            None => panic!("Bro/Sis, set $HOME to your environment variables. Otherwise use something better than M$ Shitdows <3"),
        };
        let ini_content = match fs::read_to_string(format!("{}/.config/s3-cli/config.ini", &path)).await {
            Ok(data) => data,
            Err(_) => panic!("Failed to read default configuration file. Either create it or pass different config in options"),
        };
        config = serde_ini::from_str(&ini_content)?;
    }

    if let Some(send) = args.send {
        for file in send {
            println!("{}", send_file(&config, &file).await?);
        }
    }

    if let Some(recv) = args.recv {
        for file in recv {
            println!("{}", recv_file(&config, &file).await?);
        }
    }

    if args.list {
        for file in list_objects(&config).await? {
            println!("{}", file);
        }
    }

    if args.dump {
        println!("{}", dump(&config).await?);
    }

    if let Some(archive) = args.populate {
        println!("{}", populate(&config, &archive).await?)
    }

    Ok(())
}
