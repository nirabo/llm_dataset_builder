use anyhow::{Result, anyhow};
use async_trait::async_trait;
use reqwest::Client;
use std::path::{Path, PathBuf};
use url::Url;
use regex::Regex;
use serde::Deserialize;
use walkdir::WalkDir;

#[async_trait]
pub trait DataSource {
    async fn collect(&self, output_dir: &Path) -> Result<Vec<PathBuf>>;
}

pub struct UrlSource {
    url: Url,
}

impl UrlSource {
    pub fn new(url: &str) -> Result<Self> {
        Ok(Self {
            url: Url::parse(url)?,
        })
    }
}

#[async_trait]
impl DataSource for UrlSource {
    async fn collect(&self, output_dir: &Path) -> Result<Vec<PathBuf>> {
        let client = Client::new();
        let response = client.get(self.url.as_str()).send().await?;
        let content = response.text().await?;
        
        let filename = self.url.path_segments()
            .and_then(|segments| segments.last())
            .unwrap_or("downloaded_content.txt");
            
        let output_path = output_dir.join(filename);
        std::fs::write(&output_path, content)?;
        
        Ok(vec![output_path])
    }
}

pub struct LocalSource {
    path: PathBuf,
}

impl LocalSource {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_owned(),
        }
    }
}

#[async_trait]
impl DataSource for LocalSource {
    async fn collect(&self, output_dir: &Path) -> Result<Vec<PathBuf>> {
        let mut collected = Vec::new();
        
        if self.path.is_file() {
            let filename = self.path.file_name()
                .ok_or_else(|| anyhow!("Invalid filename"))?;
            let dest_path = output_dir.join(filename);
            std::fs::copy(&self.path, &dest_path)?;
            collected.push(dest_path);
        } else if self.path.is_dir() {
            for entry in WalkDir::new(&self.path).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    let relative_path = entry.path().strip_prefix(&self.path)?;
                    let dest_path = output_dir.join(relative_path);
                    if let Some(parent) = dest_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::copy(entry.path(), &dest_path)?;
                    collected.push(dest_path);
                }
            }
        }
        
        Ok(collected)
    }
}

#[derive(Debug, Deserialize)]
struct GithubApiContent {
    name: String,
    path: String,
    #[serde(rename = "type")]
    content_type: String,
    download_url: Option<String>,
}

pub struct GitHubSource {
    owner: String,
    repo: String,
    branch: String,
    path: String,
}

impl GitHubSource {
    pub fn new(url: &str, _branch: Option<String>, _path: Option<String>) -> Self {
        let re = Regex::new(r"https://github\.com/([^/]+)/([^/]+)/tree/([^/]+)/(.*)").unwrap();
        let caps = re.captures(url).expect("Invalid GitHub URL format");
        
        Self {
            owner: caps[1].to_string(),
            repo: caps[2].to_string(),
            branch: caps[3].to_string(),
            path: caps[4].to_string(),
        }
    }

    async fn list_directory_contents(&self, client: &Client) -> Result<Vec<GithubApiContent>> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/contents/{}?ref={}",
            self.owner, self.repo, self.path, self.branch
        );

        let response = client
            .get(&url)
            .header("User-Agent", "rust-github-raw-fetcher")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to fetch directory contents: {}", response.status()));
        }

        let contents: Vec<GithubApiContent> = response.json().await?;
        Ok(contents)
    }

    fn is_supported_file(filename: &str) -> bool {
        let lowercase = filename.to_lowercase();
        lowercase.ends_with(".md") || 
        lowercase.ends_with(".txt") ||
        lowercase.ends_with(".rst") ||
        lowercase.ends_with(".markdown")
    }
}

#[async_trait]
impl DataSource for GitHubSource {
    async fn collect(&self, output_dir: &Path) -> Result<Vec<PathBuf>> {
        let client = Client::new();
        let mut collected = Vec::new();

        println!("Fetching contents from GitHub directory...");
        let contents = self.list_directory_contents(&client).await?;

        for item in contents {
            if item.content_type != "file" || !Self::is_supported_file(&item.name) {
                continue;
            }

            if let Some(download_url) = item.download_url {
                println!("Downloading: {}", item.path);
                let response = client.get(&download_url)
                    .header("User-Agent", "rust-github-raw-fetcher")
                    .send()
                    .await?;

                if !response.status().is_success() {
                    println!("Failed to download {}: {}", item.path, response.status());
                    continue;
                }

                let content = response.text().await?;
                let output_path = output_dir.join(&item.name);
                std::fs::write(&output_path, content)?;
                collected.push(output_path);
                println!("Successfully downloaded: {}", item.name);
            }
        }

        if collected.is_empty() {
            println!("No supported files found in the specified directory.");
        } else {
            println!("Downloaded {} files", collected.len());
        }

        Ok(collected)
    }
}

pub struct GitHubReleaseSource {
    repo: String,
}

impl GitHubReleaseSource {
    pub fn new(url: &str) -> Result<Self> {
        let re = Regex::new(r"https://github\.com/([^/]+/[^/]+)/releases")?;
        if let Some(captures) = re.captures(url) {
            Ok(Self {
                repo: captures[1].to_string(),
            })
        } else {
            Err(anyhow!("Invalid GitHub releases URL"))
        }
    }
}

#[async_trait]
impl DataSource for GitHubReleaseSource {
    async fn collect(&self, output_dir: &Path) -> Result<Vec<PathBuf>> {
        let client = Client::new();
        let url = format!("https://api.github.com/repos/{}/releases", self.repo);
        
        println!("Fetching releases from {}", url);
        let releases: Vec<Release> = client
            .get(&url)
            .header("User-Agent", "llm-dataset-builder")
            .send()
            .await?
            .json()
            .await?;
            
        let mut files = Vec::new();
        for release in releases {
            let filename = format!("{}.md", release.tag_name);
            let file_path = output_dir.join(&filename);
            std::fs::write(&file_path, release.body)?;
            println!("Saved release notes for version {}", release.tag_name);
            files.push(file_path);
        }
        
        Ok(files)
    }
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    body: String,
}
