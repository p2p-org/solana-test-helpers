use anyhow::anyhow;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    account_utils::StateMut,
    bpf_loader, bpf_loader_deprecated, bpf_loader_upgradeable,
    bpf_loader_upgradeable::UpgradeableLoaderState,
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair, Signer},
};
use std::{fmt, fs::File, io::Read, path::PathBuf, str::FromStr};
use subprocess::Exec;

#[derive(Debug)]
pub enum Network {
    Localhost,
    Devnet,
    Testnet,
    MainnetBeta,
    Other(String),
}

impl Network {
    pub fn rpc_url(&self) -> String {
        match self {
            Self::Localhost => String::from("http://localhost:8899"),
            Self::Devnet => String::from("https://devnet.solana.com"),
            Self::Testnet => String::from("https://testnet.solana.com"),
            Self::MainnetBeta => String::from("https://api.mainnet-beta.solana.com"),
            Self::Other(url) => url.to_owned(),
        }
    }
}

impl AsRef<str> for Network {
    fn as_ref(&self) -> &str {
        match self {
            Self::Localhost => "localhost",
            Self::Devnet => "devnet",
            Self::Testnet => "testnet",
            Self::MainnetBeta => "mainnet-beta",
            Self::Other(url) => &*url,
        }
    }
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_ref())
    }
}

impl FromStr for Network {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "localhost" => Self::Localhost,
            "devnet" => Self::Devnet,
            "testnet" => Self::Testnet,
            "mainnet-beta" => Self::MainnetBeta,
            url => Self::Other(url.to_owned()),
        })
    }
}

#[derive(Debug)]
pub struct Program {
    path: PathBuf,
    keypair_path: PathBuf,
    payer_path: PathBuf,
    keypair: Keypair,
    network: Network,
}

impl Program {
    pub fn new(
        path: impl Into<PathBuf>,
        keypair_path: impl Into<PathBuf>,
        payer_path: impl Into<PathBuf>,
    ) -> anyhow::Result<Program> {
        let keypair_path = keypair_path.into();
        let keypair = read_keypair_file(&keypair_path).map_err(|err| anyhow!("Invalid keypair file: {:?}", err))?;
        Ok(Program {
            path: path.into(),
            payer_path: payer_path.into(),
            keypair_path,
            keypair,
            network: Network::Localhost,
        })
    }

    pub fn for_network(&mut self, network: Network) -> &mut Self {
        self.network = network;
        self
    }

    pub fn pubkey(&self) -> Pubkey {
        self.keypair.pubkey()
    }

    pub fn deploy(&mut self) -> anyhow::Result<()> {
        Exec::cmd("solana")
            .arg("program")
            .arg("deploy")
            .arg("--url")
            .arg(self.network.as_ref())
            .arg("--program-id")
            .arg(&self.keypair_path)
            .arg("--keypair")
            .arg(&self.payer_path)
            .arg(&self.path)
            .join()?;

        Ok(())
    }

    pub fn deploy_if_changed(&mut self) -> anyhow::Result<()> {
        let client = RpcClient::new_with_commitment(self.network.rpc_url(), CommitmentConfig::confirmed());
        if self.is_changed(&client).unwrap_or(true) {
            self.deploy()?;
        }
        Ok(())
    }

    pub fn is_changed(&self, client: &RpcClient) -> anyhow::Result<bool> {
        let program_data = self.get_program_data(client)?;
        let orig_file = File::open(&self.path)?;
        let result = program_data
            .into_iter()
            .zip(orig_file.bytes().filter_map(Result::ok).chain(std::iter::repeat(0u8)))
            .any(|(a, b)| a != b);
        Ok(result)
    }

    pub fn get_program_data(&self, client: &RpcClient) -> anyhow::Result<Vec<u8>> {
        let program_id = self.pubkey();
        let account = client.get_account(&program_id)?;
        if account.owner == bpf_loader::id() || account.owner == bpf_loader_deprecated::id() {
            Ok(account.data)
        } else if account.owner == bpf_loader_upgradeable::id() {
            match account.state()? {
                UpgradeableLoaderState::Program { programdata_address } => {
                    let programdata_account = client.get_account(&programdata_address)?;
                    Ok(
                        programdata_account.data[UpgradeableLoaderState::programdata_data_offset().unwrap_or(0)..]
                            .to_owned(),
                    )
                },
                UpgradeableLoaderState::Buffer { .. } => {
                    Ok(account.data[UpgradeableLoaderState::buffer_data_offset().unwrap_or(0)..].to_owned())
                },
                _ => Err(anyhow!("Invalid program state")),
            }
        } else {
            Err(anyhow!("Non-program account"))
        }
    }
}
