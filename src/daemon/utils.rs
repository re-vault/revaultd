#[cfg(test)]
pub mod test_utils {
    use crate::{
        jsonrpc::UserRole,
        revaultd::RevaultD,
        threadmessages::{BitcoindMessageOut, SigFetcherMessageOut},
        RpcUtils,
    };
    use common::config::Config;

    use std::{
        fs,
        path::PathBuf,
        sync::{mpsc, Arc, RwLock},
        thread,
    };

    // Create a RevaultD state instance using a scratch data directory, trying to be portable
    // across UNIX, MacOS, and Windows
    pub fn dummy_revaultd(datadir_name: &'static str, role: UserRole) -> RevaultD {
        let repo_root = PathBuf::from(file!())
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let datadir_path: PathBuf = [repo_root.to_str().unwrap(), "test_data", datadir_name]
            .iter()
            .collect();

        let config = match role {
            UserRole::Stakeholder => "stake_config.toml",
            UserRole::Manager => "man_config.toml",
            UserRole::ManagerStakeholder => "stake_man_config.toml",
        };

        let config_path = [repo_root.to_str().unwrap(), "test_data", config]
            .iter()
            .collect();

        // Just in case there is a leftover from a previous run
        fs::remove_dir_all(&datadir_path).unwrap_or_else(|_| ());

        let mut config = Config::from_file(Some(config_path)).expect("Parsing valid config file");
        config.data_dir = Some(datadir_path);
        RevaultD::from_config(config).expect("Creating state from config")
    }

    // Get a dummy handle for the RPC calls. We don't actually test RPC calls requiring it here but
    // we need to because types.
    // FIXME: we could do something cleaner at some point
    pub fn dummy_rpcutil(datadir_name: &'static str, role: UserRole) -> RpcUtils {
        let revaultd = Arc::from(RwLock::from(dummy_revaultd(datadir_name, role)));

        let (bitcoind_tx, bitcoind_rx) = mpsc::channel();
        let (sigfetcher_tx, sigfetcher_rx) = mpsc::channel();

        let bitcoind_thread = Arc::from(RwLock::from(thread::spawn(move || {
            for msg in bitcoind_rx {
                match msg {
                    BitcoindMessageOut::Shutdown => return,
                    _ => unreachable!(),
                }
            }
        })));
        let sigfetcher_thread = Arc::from(RwLock::from(thread::spawn(move || {
            for msg in sigfetcher_rx {
                match msg {
                    SigFetcherMessageOut::Shutdown => return,
                }
            }
        })));

        RpcUtils {
            revaultd,
            bitcoind_tx,
            bitcoind_thread,
            sigfetcher_tx,
            sigfetcher_thread,
        }
    }
}
