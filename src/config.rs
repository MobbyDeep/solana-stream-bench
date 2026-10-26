use crate::proto::geyser::CommitmentLevel;
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Deserialize, Serialize)]
pub struct ConfigToml {
    pub config: Config,
    pub endpoint: Vec<Endpoint>,
    #[serde(default)]
    pub backend: BackendSettings,
    #[serde(default)]
    pub output: OutputSettings,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub transactions: i32,
    pub account: String,
    pub commitment: ArgsCommitment,
    #[serde(default)]
    pub yellowstone_accounts_bench_enabled: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Endpoint {
    pub name: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_token: Option<String>,
    pub kind: EndpointKind,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct OutputSettings {
    pub txt_file_output: bool,
    pub directory: String,
    pub timestamp_format: String,
}
impl Default for OutputSettings {
    fn default() -> Self {
        Self {
            txt_file_output: true,
            directory: "./results".into(),
            timestamp_format: "%Y-%m-%d_%H-%M-%S".into(),
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct BackendSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EndpointKind {
    Yellowstone,
    Arpc,
    ArpcV3,
    Thor,
    Shredstream,
    Shreder,
    Jetstream,
    #[serde(alias = "udpshredstream")]
    UdpShredstream,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ArgsCommitment {
    #[default]
    Processed,
    Confirmed,
    Finalized,
}

impl From<ArgsCommitment> for CommitmentLevel {
    fn from(commitment: ArgsCommitment) -> Self {
        match commitment {
            ArgsCommitment::Processed => CommitmentLevel::Processed,
            ArgsCommitment::Confirmed => CommitmentLevel::Confirmed,
            ArgsCommitment::Finalized => CommitmentLevel::Finalized,
        }
    }
}

impl ArgsCommitment {
    pub fn as_str(&self) -> &'static str {
        match self {
            ArgsCommitment::Processed => "processed",
            ArgsCommitment::Confirmed => "confirmed",
            ArgsCommitment::Finalized => "finalized",
        }
    }
}

impl EndpointKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EndpointKind::Yellowstone => "yellowstone",
            EndpointKind::Arpc => "arpc",
            EndpointKind::ArpcV3 => "arpcv3",
            EndpointKind::Thor => "thor",
            EndpointKind::Shredstream => "shredstream",
            EndpointKind::Shreder => "shreder",
            EndpointKind::Jetstream => "jetstream",
            EndpointKind::UdpShredstream => "udpshredstream",
        }
    }
}

impl ConfigToml {
    pub fn load(path: &str) -> Result<Self> {
        let content =
            fs::read_to_string(path).with_context(|| format!("Failed to read config {}", path))?;
        // Parser diagnostics may include authentication values from the input.
        let config: Self = toml::from_str(&content).map_err(|_| anyhow!("Invalid config: expected [config] transactions/account/commitment and [[endpoint]] name/url/kind; check the configuration example in README.md"))?;
        config.validate()?;
        Ok(config)
    }

    pub fn create_default(path: &str) -> Result<Self> {
        let default_config = ConfigToml {
            config: Config {
                transactions: 1000,
                account: "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA".to_string(),
                commitment: ArgsCommitment::Processed,
                yellowstone_accounts_bench_enabled: false,
            },
            endpoint: vec![
                Endpoint {
                    name: "Provider A".to_string(),
                    url: "https://provider-a.example.com:443".to_string(),
                    x_token: None,
                    kind: EndpointKind::Yellowstone,
                },
                Endpoint {
                    name: "Provider B".to_string(),
                    url: "https://provider-b.example.com:443".to_string(),
                    x_token: None,
                    kind: EndpointKind::Yellowstone,
                },
            ],
            backend: BackendSettings::default(),
            output: OutputSettings::default(),
        };

        let toml_string = toml::to_string_pretty(&default_config)
            .context("Failed to serialize default config")?;
        fs::write(path, toml_string)
            .with_context(|| format!("Failed to write default config {}", path))?;

        Err(anyhow!(
            "Created {path}; replace the placeholder endpoint addresses before running Solana Stream Bench"
        ))
    }

    pub fn validate(&self) -> Result<()> {
        use std::collections::HashSet;
        anyhow::ensure!(
            !self.endpoint.is_empty(),
            "At least one endpoint is required"
        );
        anyhow::ensure!(
            self.config.transactions >= 0,
            "transactions must be >= 0 (0 runs until Ctrl+C)"
        );
        self.config
            .account
            .parse::<solana_pubkey::Pubkey>()
            .map_err(|_| anyhow!("Invalid config.account public key"))?;
        let mut names = HashSet::new();
        for (i, endpoint) in self.endpoint.iter().enumerate() {
            anyhow::ensure!(
                !endpoint.name.trim().is_empty() && !endpoint.name.chars().any(char::is_control),
                "Endpoint names must be nonempty without control characters"
            );
            anyhow::ensure!(names.insert(&endpoint.name), "Duplicate endpoint name");
            if endpoint.kind == EndpointKind::UdpShredstream {
                let address = endpoint.url.parse::<std::net::SocketAddr>().map_err(|_| {
                    anyhow!(
                        "UDP endpoint {} requires a bare IP:port bind address",
                        i + 1
                    )
                })?;
                anyhow::ensure!(
                    address.port() > 0 && address.port() < u16::MAX,
                    "UDP bind port must be in 1..65535"
                );
                continue;
            }
            let url = url::Url::parse(&endpoint.url)
                .map_err(|_| anyhow!("Endpoint {}: invalid URL", i + 1))?;
            anyhow::ensure!(
                ["http", "https"].contains(&url.scheme()) && url.host_str().is_some(),
                "Endpoint {}: invalid URL scheme or missing host",
                i + 1
            );
            anyhow::ensure!(
                url.username().is_empty() && url.password().is_none(),
                "Use x_token instead of URL credentials"
            );
            if let Some(token) = &endpoint.x_token {
                anyhow::ensure!(
                    token.bytes().all(|b| (32..127).contains(&b)),
                    "x_token must contain printable ASCII"
                );
            }
        }
        anyhow::ensure!(
            !self.output.directory.is_empty(),
            "output.directory must be nonempty"
        );
        anyhow::ensure!(
            !self.output.timestamp_format.is_empty()
                && !chrono::format::StrftimeItems::new(&self.output.timestamp_format)
                    .any(|v| matches!(v, chrono::format::Item::Error)),
            "Invalid output.timestamp_format"
        );
        anyhow::ensure!(
            crate::report::safe_component(
                &chrono::Utc::now()
                    .format(&self.output.timestamp_format)
                    .to_string()
            ),
            "Timestamp format must produce a safe filename"
        );
        Ok(())
    }

    pub fn load_or_create(path: &str) -> Result<Self> {
        if Path::new(path).exists() {
            Self::load(path)
        } else {
            Self::create_default(path)
        }
    }
}
